# 离线 nickname 与语言变量修复

同时更新 Rust 后端与配套 Velocity 插件。完成离线登录后使用 `/nickname <昵称>`、`/nick <昵称>` 或 `/aihmc nickname <昵称>`，重新连接生效。登录器、注册、登录及改密始终使用原账户名。

## 身份与重名

- 昵称仅改变 GameProfile.name；UUID 读取已有 `offline.uuid`，不按昵称重新计算，不修改 `userlink.new_uuid` 或迁移玩家数据。
- 使用现有 `namelink` 保存昵称，无新增表或 DDL；旧账户缺少映射时沿用其已存 UUID 补齐。
- 查询本地资料/映射及启用的正版、外置名称来源。冲突时分配 `AIHMC_` 加 8 位小写字母/数字；已注册账户持久保存，重登不会重复随机。
- 离线先占名、之后在线账户进服时，保留在线名称，只给离线账户换随机昵称，双方 UUID 均不改变。
- 当前已连接会话不强制改名，重新连接时应用新昵称；未注册玩家首次资料使用原登录名生成的离线 UUID，注册仍沿用该 UUID。
- 数据库唯一约束和事务保证不同账户不覆盖他人映射；已验证并发同名申请。数据库/上游失败显式返回错误，不当作名称可用。

## API

`POST /offline/reg` 增加可选 `nickname` 字符串；省略/null 默认原账户名。保持原成功消息，昵称格式错误 400，类型错误 422。

`GET /offline/profile/{username}` 使用既有 Bearer 服务令牌，返回：

```json
{
  "id": "原账户无连字符UUID",
  "name": "实际进服昵称或随机名",
  "username": "原账户名",
  "registered": true,
  "properties": []
}
```

此接口只解析资料，不验证密码、不授予登录状态。插件在 PreLogin 取资料，在 GameProfileRequestEvent 应用 name/id，离线账户仍需密码认证；正版登录状态只在实际在线认证完成后的资料事件授予。

`POST /profile/rename` 对离线 UUID 设置昵称，支持带/不带连字符；在线账户保留原改名行为。

## 语言变量核对

| 模板/变量 | 结果 |
| --- | --- |
| `in-cooldown` / `%remaining%` | 修复 6 处未传值，传入实时向上取整剩余分钟数 |
| 冷却模板 `%minutes%` | 同时提供配置的完整冷却分钟数 |
| `changepw-log-success` / `%player%` | 修复绕过语言文件的硬编码日志，读取模板并传实际玩家名 |
| 超时 `%time%` | 已有注册/登录路径传入对应时间，保留 |
| 用法/错误 `%usage%`、`%error%` | 现有路径传值，保留 |
| 重载 `%executor%`、`%error%` | 现有重载路径传值，保留 |
| 迁移 `%old_uuid%`、`%new_uuid%`、`%player%`、`%uuid%`、`%error%` | 现有迁移/踢出路径传值，保留 |
| 重置名 `%player%`、`%uuid%` | 现有重置日志传值，保留 |
| 新昵称反馈 `%nickname%`、`%uuid%` | 显示实际分配名、原 UUID 和重连生效提示 |
| 旧 max-retry-reached、名称冲突拦截提示 | 无现行调用/被昵称流程替代，保留历史键，不宣称正在触发 |

冷却到期后旧重试计数会立即重启完整冷却，现已修复；可控时钟测试验证剩余 10→9→0 分钟及到期可重试。消息加载合并内置默认值与用户 overrides，保留已有定制并补齐新增键，不覆盖语言文件。

## 验证与交付边界

本地仅 Windows x64。自动覆盖真实回环 HTTP、现有 MariaDB 隔离库、并发昵称、旧 UUID 数据哨兵、Velocity API 事件、原账户登录/改密及语言变量。

`scripts/test-velocity-backend.py` 将插件测试接到真实 Rust 进程及隔离库。这是 API/事件集成，不启动实服或 Minecraft 客户端。

- 实服验证：**人工验收／SKIPPED（用户要求跳过）**。
- 启动客户端认证：**人工验收／SKIPPED（用户要求跳过）**。
- Actions/Linux：**NOT RUN（仅手动触发，本次不运行）**。

人工项不记为 PASS，不作为阻塞条件。后端分支 `aihmc-auth`；插件独立检出于忽略的 `test-output/velocity-plugin`，分支 `offline-nickname`。发布时同时更新两者，沿用现有 Velocity UUID 转发配置。

## 本次交付结果（2026-11-14）

- Windows Rust：fmt、Clippy -D warnings、8 单元测试、38 项基线/数据库/HTTP/昵称集成全部 PASS。
- 配套插件：现有 JDK21 编译，7 项测试 PASS，0 失败、0 跳过；其中 1 项连接实际 Windows ZIP 解压二进制及现有 MariaDB 隔离库。
- 已验证转发/皮肤等原 GameProfile 属性保留；账户名、原 UUID、昵称冲突、重连登录、改密和语言变量均有自动证据。
- 日志：test-output/logs/windows-integration.log、velocity-backend-integration.log；插件 build/test-results/test/。
- Windows ZIP：test-output/artifacts/aihmc-auth-windows-x64.zip，SHA256 276e7554c68446a3af14bd36767e7c1c5e2bbba38db307cc4ebd022b106edd0e。
- 插件 JAR：test-output/artifacts/aihmc-auth-velocity.jar，SHA256 2ad8293b4f42ab93f0258bfd8258854b677cc72697dd4989f7836866ffc08fe7。
- 曾有属性测试夹具使用 API 不接受的 null 签名；日志定位后修正为合法空签名，最终全部通过，未为测试改变生产规则。
- Gradle 仍输出原有弃用/unchecked 编译提示；不在本轮扩大清理范围。
- 实服和客户端认证依用户要求人工 SKIPPED；无剩余代码实施。提交推送核对结果记录于 AGENT_HANDOFF.md。

提交推送已核对：后端实施提交 `66453f5`（aihmc-auth）；插件实施提交 `ddf0290`（offline-nickname）。两个远端均与本地对应实施提交一致，后续仅同步交接确认文档；无剩余实施。

## 2026-11-14 名称重置与离线重连修复

- 正版/外置重置从 `last_source` 对应的原始资料接口取得原名，不再从可能被昵称覆盖的 `users.username` 恢复。按 `userlink` 解析源 UUID 和进服 UUID，保留关联；源不可用、UUID 不符、多个来源关联或在线名称冲突时明确失败，不返回虚假成功。
- 重置支持映射缺失时插入昵称；在线原名被离线昵称占用时，只为离线账户分配新名，双方 UUID 不变。离线重置恢复原账户名（冲突仍由名称分配器处理）。
- 插件不再反序列化 `Void.class`；其他 JSON 解析失败通过 Future 报错，不继续当作成功响应。
- 已复现离线昵称绕过场景：名称查询属于 A、客户端 UUID 查询属于 B，两次 200 被旧代码误判成在线连接，跳过离线昵称应用。现在核对名称对应的源 UUID 与客户端 UUID；仍必须通过后续在线认证才能获得正版登录状态。
- 在应用离线资料时记录 account/nickname/uuid，便于区分代理已应用昵称与后端服转发问题；不记录密码、Token。
- 并发回归捕获 InnoDB 1213 死锁，昵称分配现在在原有 32 次上限内回滚重试，死锁本身不会触发随机改名。
- 改名和重置仍在重新连接后生效；`/login` 不接受昵称参数，启动器继续使用原账户名。
- 回归覆盖真实 Windows Rust 进程、现有 MariaDB 隔离库、HTTP 认证源夹具、Velocity API 事件及命令类；认证源响应、连接对象为测试夹具，未启动真实 Velocity 服务器或 Minecraft 客户端。实服及客户端：SKIPPED（用户要求）；Actions/Linux：NOT RUN。

验证结果：Windows fmt/Clippy、8 Rust 单元测试、44 项业务/数据库回归及 ZIP 解压启动检查通过；昵称并发额外 5 轮通过；插件 JDK21 编译及 12 项测试通过（0 失败/跳过）。旧在线分流判断在真实后端调用链回归中失败，修复后通过。证据见 `test-output/logs/name-fix-windows.log`、`nickname-real-chain-red.log`、`nickname-real-chain.log`、`nickname-concurrency.log`。本地新产物位于 `test-output/artifacts/`，无需数据库 DDL；部署需同时更新后端及插件。

## 认证源重名检测开关

在 `config.toml` 顶层（任何 `[表]` 之前）加入：

```toml
check_auth_source_names = false
```

默认值为 `true`，旧配置省略时保持原行为。`false` 时离线注册、昵称修改、重连资料解析及随机替换名分配，只检查本地 `namelink.username` 和 `users.username`（排除相同 UUID）；冲突仍分配 `AIHMC_` 加 8 位随机字符，已注册账户持久保存，UUID 不变。在线主动改名同样跳过外部名称占用查询，本地在线名称优先规则保持不变。

此开关仅控制名称可用性检查；正常认证源登录查询、会话验证和重置原名读取认证源资料继续执行。修改配置后重启后端生效，插件无需更新。未配置的认证源名称不再提前保留；之后在线账户进服时，仍按在线名称优先规则处理本地冲突。
