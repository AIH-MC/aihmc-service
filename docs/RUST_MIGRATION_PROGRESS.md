# Rust 迁移实施记录

日期：2026-11-14。实施依据：`../RUST_MIGRATION_PLAN.md`。

用户授权按 R0 至 R6 顺序实施，每阶段验证后提交推送，再继续下一阶段。本地仅构建和测试 Windows x64。最新更正：Actions 完全交由人工，Agent 禁止运行，不得作为阶段或交付阻塞条件；Linux musl 和远端产物验收统一记录非阻塞 MANUAL/PENDING。

## R0 — 自动验证通过

Go 源基线：`cc458ab7d709323bdf4b9ca6f7ac1c857c4cfa09`。归档候选文件及其工作区 SHA256 见 `rust-go-baseline-manifest.json`；清单涵盖 Go 源码、模块文件、测试、参考证据、模板、历史计划、README 与原 Action。最终归档时须逐项核对，不能归档实际配置、秘密或运行数据。

环境：Go 1.26.4，Windows amd64；已安装 Rust 1.95.0 与 Windows MSVC target；复用系统正在运行的 MariaDB。没有安装新数据库。使用现有受控连接信息和新生成的测试 RSA 密钥建立被忽略的 `.local/rust-migration/config.toml`，数据库固定为 `aihmc_migration_test`，UCenter 和管理密钥为合成值，不选用业务数据库。

验证结果：

- `go test -count=1 -p 1 -v ./...`：PASS，无测试跳过；包含真实隔离数据库、HTTP、事务、并发登录/刷新、固定协议与十组头像像素验证。
- `go build -trimpath -o .local/rust-migration/aihmc-api-go.exe ./cmd/aihmc-api`：PASS，Windows 基线程序保留供后续对照。
- 基线程序 `--check-config --config .local/rust-migration/config.toml`：PASS，配置、密钥、材质目录及数据库连接通过。
- 可选的 Go 生成令牌交由 PHP 执行检查未运行：未配置 PHP 可执行文件及隔离旧项目。PHP 固定向量由 Go 验证通过；不将其描述为本轮 PHP 双向联调 PASS。

诊断记录：受限沙箱首次执行时数据库和回环 HTTP 连接失败，正常网络权限复验后消失；正常权限首轮默认包并行测试触发共享隔离库的跨包死锁，使用 `-p 1` 后通过。包内并发用例仍执行，未修改 Go 源码或添加生产重试。后续同库对照及集成检查采用包间串行，避免测试间干扰。

本地完整日志位于被忽略的 `.local/rust-migration/logs/r0-go-tests-serial.log`、`r0-go-build.log`、`r0-go-config.log`；摘要及日志哈希见 `rust-r0-verification.json`。

配置复用核对结论：

远端检查发现后续提交 `846b46715300bbf90fc653d46704c701df6bba37` 已注册 `.github/workflows/rust.yml`，本地已安全快进。该手动工作流 checkout 的是 `aihmc-auth` 分支，不能作为 mc-auth 当前源码的构建证据。保留此已有跨分支入口；本迁移在 build.yml 中构建当前 mc-auth 的 Rust，不改动其他分支的发布行为。

1. 相邻项目已有三平台 runner/target、Rust 1.95.0、fmt/clippy/test/release 脚本，复用编译配置；不复制其业务代码或运行配置。
2. 适配二进制名为 aihmc-api，去除其 assets 目录依赖，临时阶段工作目录为 rust/，最终阶段切回仓库根目录。
3. 保留现有 push/PR/手动触发，将 Rust job 合并现有 build.yml，Go job 在迁移期间仍保留。
4. 按 omni-morph 各分支约定设置只读权限、checkout 凭据、并发组、矩阵独立失败、日志、明确产物选择与保留期；不复制 Java/Gradle 设置。
5. Rust 脚本需逐命令检查退出、保留独立日志、可靠传播 smoke 失败、解包正式包验证并生成 SHA256。

R0 检查点：`25eb2048556ea6fad06212a7acce7b7c4824d04b` 已提交、推送并核对远端一致。

生产、真实账号和客户端验收：MANUAL/PENDING，非阻塞。

## R1 — 本地自动验证通过并已推送，Actions 非阻塞人工验收

新增 `rust/` 工程，复用 Rust 1.95.0 与现有依赖选型；实现现有 TOML 结构、未知字段和非法值拒绝、配置目录相对路径、秘密末尾换行处理、PKCS1/PKCS8 私钥与 PKCS1/PKIX 公钥加载匹配、材质目录探测、CLI、真实 MySQL 连接、结构化日志和有界优雅退出。当前仅开放原有根路径提示，认证和材质业务尚未迁移，阶段包 README 明确说明此状态。

用户要求 Linux 直接用 musl，已将两个目标设置为 x86_64-unknown-linux-musl / aarch64-unknown-linux-musl；原生 runner 安装 musl-tools，链接和 C 依赖使用 musl-gcc，脚本以 readelf 检查架构和无动态依赖。Windows 仍为 x86_64-pc-windows-msvc。本地没有编译 Linux。

build.yml 保留迁移期 Go job，增加当前源码的 Rust 三平台 job，现有 checkout aihmc-auth 的跨分支 rust.yml 未改。用户随后明确提交不应自动触发 Actions，现已移除 push/PR，只保留 workflow_dispatch，后续远端验证全部交由人工，Agent 不启动。使用前台脚本，独立保存检查、测试、release、打包和解包冒烟日志。核对固定版本 upload-artifact/action.yml 后确认 archive:false 仅支持单文件，因此压缩包及 SHA256 分别上传。

本地验证：fmt、Clippy（-D warnings）、8 项 Rust 测试、Windows release、白名单打包解包和 CLI 冒烟均 PASS；解包程序连接现有隔离库的 --check-config、根路径 GET/POST、端口冲突失败、不可用数据库失败以及 Windows Ctrl+Break 正常退出均 PASS。证据和日志哈希见 rust-r1-verification.json。配置文件/密钥测试均使用临时合成资料。

R1 验证不代表完整业务已迁移。六表操作、事务及查询时限/连接池空闲控制在 R2 实现；完整路由及 HTTP 读写时限在 R3 实现；材质与删除在 R4 实现。按用户最新指示，R1 本地自动验收和提交推送已满足进入 R2 条件，远端为非阻塞 MANUAL/PENDING。

R1 实现已提交推送 `5c308b1446126370fa23579541770287fb774dff`，远端一致。自动触发的 run 43579298110 已按用户要求手动停止，确认终态 completed/cancelled；其中 ARM64 musl job 已通过，但整体取消不记为三平台 PASS。此后不再运行 Actions。Windows PE 依赖检查确认需要 VCRUNTIME140.dll，最终部署说明须列出 x64 VC++ 运行库要求。

仅手动触发与非阻塞规则修正已提交推送 `734f5b9a6a5e1bc1b2beceec988574c8614fb5dc`，核对远端一致。两个工作流均仅声明 workflow_dispatch。

## R2 — 本地自动验收通过

实现六表读取与写入、旧身份字段映射、令牌绑定及 clientToken 范围刷新、登录记录与清理、材质记录、域名和来源排序、事务提交回滚及事务/查询期限。保留原有已存在身份不重建、重复绑定不替换 accessToken、共享 clientToken 在唯一约束下失败回滚的行为。连接池按配置限制闲置连接，max_idle=0 实际关闭已释放连接。

协议模块复用原 PHP 固定向量，迁移 accessToken、refreshToken、profileId、随机标识、PHP 字符串真值、JSON 字段省略与空数组、Base64 材质字符串及 SHA1/RSA PKCS#1 v1.5。新增仅处理合成资料的 Go 测试桥 `tests/tools/protocol-bridge/`，直接调用原 Go auth.Sign 并验证 Rust 签名，不改 Go 生产实现。

UCenter 使用带正常证书验证的 Rustls/Reqwest；保留 endpoint 与路径直接拼接，支持 /api/?/token 风格入口、签名头、token→登录→token→资料查询顺序、数字/数字字符串以及无跳转行为；总序列期限覆盖全部请求。错误区分上游失败与账号密码不正确。

SQLx 0.9 首轮编译要求动态 SQL 实现 SqlSafeStr，已改为 QueryBuilder、固定 SQL 常量和参数绑定，未降低版本或绕过该检查。测试 schema 仅来自编译时内嵌的受控测试文件，执行前强制验证目标为 aihmc_migration_test。

本地验证结果：

- Windows `cargo test --locked --target x86_64-pc-windows-msvc -- --include-ignored --test-threads=1`：15 PASS，0 ignored，含显式启用的真实数据库和 Go/Rust 双向验签。
- 真实隔离库覆盖 NULL/空串映射、身份唯一冲突、绑定保留旧令牌、成功刷新及共享 clientToken 刷新失败、事务和删除回滚、成功删除范围、失效/退出、来源顺序、登录记录清理、行锁等待超时及 max_idle=0。
- UCenter 真实回环 HTTP 覆盖签名及查询入口、邮箱模式、四次请求顺序、错误密码、跳转不跟随、非法 JSON、上游错误和总期限。
- Windows fmt、Clippy -D warnings、release、压缩包 SHA256、解包 CLI 冒烟，以及解包程序真实配置/数据库检查、根路径请求、监听冲突、数据库失败、正常退出均 PASS。

证据见 `rust-r2-verification.json`。通用 CI 的默认测试命令将 3 个需要本地数据库/Go 桥的用例显式标为 ignored；本轮已通过上述 include-ignored 命令执行全部用例，不能用默认测试摘要替代完整验收。Actions、Linux musl、真实账号和客户端仍为非阻塞 MANUAL/PENDING。R3 至 R6 待实施。
