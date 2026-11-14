# Agent Handoff

- 当前持久目标实施完成：后端与 Velocity 插件离线 nickname、原账户登录、原 UUID 保留、在线名称优先/离线持久随机名、语言变量修复。
- 用户授权：完整修改配套 AIH-MC/aihmc-auth；语言变量自行核对，不再询问。
- 后端：aihmc-auth（原 HEAD 2fa037a），origin AIH-MC/aihmc-service；插件：test-output/velocity-plugin 独立仓库，offline-nickname 分支，origin AIH-MC/aihmc-auth。
- 最终提交推送：后端实施 66453f585c524bd3e7f1de28f7883040c4537375；插件实施 ddf02907ee291e09399e14d976f3e8d59671c89e。两者均已推送并核对远端一致，两个工作区干净；本交接确认作为文档提交同步。
- UUID 从已有 offline.uuid 读取，昵称存 namelink；无 DDL、不改 userlink 映射；原账户仍负责登录/注册/改密。
- 插件在 GameProfileRequestEvent 应用 name/id，保留原 GameProfile 属性；已注册昵称随机名持久化，重连生效。
- 语言修复：6 处冷却 remaining/minutes，改密 player 日志模板，用户 overrides 合并默认消息，冷却到期不重启。
- 最终 Windows：fmt、Clippy、8 Rust 单元 + 38 驱动测试 PASS；插件 JDK21 编译 + 7 测试 PASS，0 失败/跳过；包含实际 ZIP 解压二进制、Rust HTTP、MariaDB 和真实 Velocity API 事件联动。
- 测试失败历史：属性夹具 null 签名非法已修正，最终全通过；Gradle 原有弃用/unchecked 提示未扩大清理。
- 证据：docs/OFFLINE_NICKNAME.md、test-output/logs/windows-integration.log、velocity-backend-integration.log、插件 build/test-results/test/。
- 发布包：test-output/artifacts/aihmc-auth-windows-x64.zip（SHA256 276e7554c68446a3af14bd36767e7c1c5e2bbba38db307cc4ebd022b106edd0e）、aihmc-auth-velocity.jar（2ad8293b4f42ab93f0258bfd8258854b677cc72697dd4989f7836866ffc08fe7）。
- 手工状态：实服验证、启动客户端认证均为人工验收／SKIPPED（用户明确要求跳过），不得作为阻塞；Actions/Linux NOT RUN，本地仅 Windows x64。
- 私有配置始终忽略；未触碰业务数据库；随机隔离库和自动服务已清理；legacy/python 原业务源码不变。
- 最终状态：持久目标完成，无剩余代码实施或推送工作；正常交付，不等待人工验收，不触发 Actions/实服/客户端。

## 2026-11-14 Actions 调度诊断

- 用户截图并未进入编译；当前选择旧 Go 工作流 build.yml，却指定 aihmc-auth 分支，该分支只有 rust.yml，因此 dispatch unavailable。
- GitHub API 核对：默认分支 mc-auth 只有 .github/workflows/build.yml；aihmc-auth 只有 .github/workflows/rust.yml；登记工作流列表仅旧 Go Build Linux x86_64。
- Rust 工作流含 workflow_dispatch，但未进入默认分支，尚无可用的 GitHub UI 手动入口。官方文档要求手动调度工作流存在于默认分支。
- 最小修复范围：向默认 mc-auth 分支补同名 rust.yml 手动入口，保留现有 Go 工作流；之后选 Rust manual build 和 aihmc-auth 分支运行。
- 本次只读诊断远端，未改工作流/默认分支，未运行 Actions 或构建；仅补充本交接记录，尚未提交。

## 2026-11-14 手动入口补齐交付

- 用户已授权补齐默认分支入口；本任务完成，无剩余实施。
- 默认 mc-auth 新增 .github/workflows/rust.yml，提交 846b46715300bbf90fc653d46704c701df6bba37 已推送并核对远端一致；只新增此文件，原 Go build.yml 未修改。
- Rust 入口仅 workflow_dispatch，固定检出 aihmc-auth；保留 Linux x64、Linux ARM64、Windows x64 三目标。
- GitHub 登记确认：workflow id 418148898，Rust manual build，active；入口提交对应 Actions runs=0。使用 [skip ci] 避免默认分支 Go push 工作流自动构建。
- 验证：YAML/触发器/目标架构/checkout ref、Go 工作流字节保持及 git diff --check 通过；未运行构建、Actions、实服或客户端。
- 当前用户工作区仍为 aihmc-auth；仅同步 README 与本交接记录到该分支，原功能实施提交不变。
- 手动入口：https://github.com/AIH-MC/aihmc-service/actions/workflows/rust.yml 。刷新 Actions 后选 Rust manual build，按需由用户手动运行。

## 2026-11-14 Linux glibc 兼容修复

- 用户报告 Linux 二进制启动缺少 GLIBC_2.32、2.33、2.34；旧工作流在 Ubuntu 24.04 构建 GNU 目标，产物依赖构建环境 glibc。
- 将 Linux x64/ARM64 改为 musl 静态目标；在各自原生 runner 安装 musl-tools，指定 musl-gcc 和 +crt-static。Windows 目标保持不变。
- ci.ps1 在 Linux release 后通过 readelf 检查 ELF，不允许 INTERP、NEEDED 或 GLIBC_ 版本依赖，检查日志随产物上传。保留解压包 HTTP smoke。
- 同步更新默认 mc-auth 的手动入口（仍固定 checkout aihmc-auth），原 Go 工作流不改。两分支均使用 [skip ci] 提交并推送；不触发 Actions。
- 验证：两份 YAML 手动触发器/目标/工具链/checkout 检查、PowerShell 语法、git diff --check PASS；Windows fmt、Clippy、8 单元测试、release、ZIP 解压 HTTP smoke PASS。
- 首次沙箱内 HTTP smoke 失败；允许本机监听后完整复跑 PASS，日志 test-output/logs/glibc-windows-validation.log。
- Linux 编译及静态检查的实际执行 NOT RUN（用户要求本地只 Windows、Actions 不运行）；Linux 服务器及客户端人工验收 SKIPPED（用户要求），不作为阻塞。
- 修复实施完成；用户下一步手动运行 Rust manual build，用新 Linux 包替换旧二进制，保留配置/密钥/数据库。旧包不会自动改变，无需更换系统 libc。
