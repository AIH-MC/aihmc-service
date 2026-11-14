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

R2 检查点：`4a66088442717db4a9fe973b833b587614555327` 已提交推送，远端核对一致。

## R3 — 本地自动验收通过

实现认证、刷新、验证、失效、退出、角色查询、join/hasJoined、元信息与默认路由的 HTTP handler；保持原始 URI 匹配、查询参数最后一个值、unsigned 字符串真值、二次查询解码、JSON 空值和空体、空数组及字段省略。X-Real-IP 仅信任回环 TCP 对端，包含 IPv4-mapped IPv6 的正确处理。

认证事务保持本地身份锁、重复绑定令牌、刷新后二次锁校验及已证实的死锁重试范围；上游请求在重试之外执行。事务 API 允许业务错误向外传播并回滚，避免把无效令牌当作正常提交结果。

使用 Hyper HTTP/1 和每连接时钟落实原有 ReadHeaderTimeout、整请求 ReadTimeout、从头部读取完成开始的 WriteTimeout，以及由 ReadTimeout 派生的空闲期限。不能把请求头期限用于 keep-alive 空闲等待，也不能在无缓冲 TCP flush 时提前取消尚未写响应的处理器。真实 TCP 对照确认慢头、慢体、空闲复用、写超时和已提交数据库副作用均与 Go 一致。正式二进制已使用该传输层，正常 Windows 控制台退出通过。

验证证据：

- 17 项 Windows Rust 全量测试（含 3 项显式启用的数据库/跨语言用例）PASS。
- `scripts/verify-contract.py` 通过真实 Go/Rust handler 进程分别执行 45 项请求，比较状态、Content-Type、JSON、204 空体及最终数据库状态；数据库每轮复位为相同隔离初始状态。八请求并发首次登录、并发刷新、Go 写 Rust 读/Rust 写 Go 读以及新 Rust 进程读取旧令牌和身份均 PASS。
- 固定材质依赖只用于该受控 R3 对照，不是完整材质验证。随机 UUID、令牌、生成 clientToken 和当轮时间按明确字段映射归一化；材质值先解码再比较字段，已签名响应由原 Go 密钥实现验证，不把签名字符串差异直接忽略。
- `scripts/verify-timeouts.py` 对照两种实现的慢请求头、整请求期限、空闲期限、响应写期限和超时后的已提交记录，全部 PASS。
- Windows Black/rustfmt、Clippy -D warnings、release、白名单发布包解包、CLI 冒烟、真实配置/数据库检查、根路径和正常退出均 PASS。

过程中并发场景曾在 Go 同样返回 500；最小复现和脱敏错误类别定位到 Python 受控 UCenter 默认 backlog=5，改为 64 后保留原八请求压力，Go/Rust 均通过，未修改 Go 生产代码或减少并发数。keep-alive 期限差异已由独立 TCP 用例复现并修正。

完整证据见 `rust-r3-verification.json`、`rust-r3-contract-comparison.json`、`rust-r3-timeouts.json`。业务 HTTP 以独立夹具进程验证，正式入口当前仍仅开放根路径；R4 完成实际材质依赖后统一组装完整业务入口，不宣称当前阶段包已可替换完整 Go 服务。Actions/Linux 为非阻塞 MANUAL/PENDING，不运行、不等待。R4 至 R6 待实施。

R3 检查点：`5ff72c38f54699da3949dcccd807b344eadc07b1` 已提交推送，远端核对一致。

## R4 — 本地自动验收通过

迁移实际材质服务：按主键顺序查询来源，保持 CSL 用户名编码、slim/default 判断、命名材质下载和验证、Mojang/Crafatar 补齐、原始字节 SHA256、共享远程期限及部分结果保留。远程材质缺失不拒绝有效登录，真实数据库和文件系统失败仍返回错误。临时文件和持有同一锁的原子替换/读取保留并发发布完整性；Unix 材质文件保持 0644。

头像迁移保留 GD 面积积分、两次除法、7 位 alpha 舍入和帽子叠层；模型识别保留越界零值及调色板原始索引比较。新增实际头像及管理删除路由，保留原字段 prarms.uid、空/错误密钥、缺失用户、删除范围和事务回滚。正式 main 已接入所有真实业务依赖和完整 handler，不再只开放根路径。

验证：

- Windows 全量 27 项 Rust 测试 PASS，0 ignored；十组原 GD 参考逐像素一致，模型与调色板场景通过。
- 7 项材质服务用例覆盖来源顺序、中文/空格编码、缺失披风回退、坏 JSON/PNG/文件名/跳转、部分超时、真实数据库错误不可忽略、八路并发发布、远程 PNG 原样转发、本地头像像素和文件系统失败。
- 使用原 Go 正式程序和最终 Rust release 压缩包解包程序，各执行 63 项真实 HTTP 请求；响应、最终数据库及材质文件哈希经明确归一化后一致。实际下载和头像逐像素检查通过，删除后其他用户仍可验证。
- 在隔离测试库临时创建删除故障触发器，故意在删除最后一张表时报错；HTTP 500 后资料、令牌、材质、会话全部恢复到原状态，令牌仍可用。触发器在 finally 中移除，随后正常删除成功。没有在业务库创建触发器或操作用户。
- 八并发登录、并发刷新、跨语言令牌互认、新 Rust 进程读取旧身份与令牌、配置和数据库检查、端口冲突、正常 Windows Ctrl+Break 退出均 PASS。
- Black/rustfmt、Clippy -D warnings、release、打包和解包 CLI 冒烟 PASS。

证据见 `rust-r4-verification.json`、`rust-r4-contract-comparison.json`。日志中的重复 join、损坏远程头像和故障删除 ERROR 是有意验证的失败路径，预期分别为 HTTP 500，并非未处理失败。Actions/Linux、真实账号/客户端和生产操作继续为非阻塞 MANUAL/PENDING。下一阶段为 R5 Go 归档与 Rust 根目录提升。

R4 检查点：`c01473979facddf2d09ce9816a818c79fa5663b8` 已提交推送，远端核对一致。

## R5 — 本地自动验收通过

Rust 提升至根目录，活动构建、测试及发布脚本使用根目录 Cargo 工程。Go 完整归档至 archive/go，包含 58 个原始基线文件和 2 个迁移校验助手；60 个 SHA256 全部匹配清单。旧 Go workflow 仅作为归档保存，PHP 归档未修改。

独立 Go 归档33个顶层测试及Windows编译通过。根目录Rust全量27项测试（含全部数据库和跨语言用例）、rustfmt、Clippy -D warnings、Windows release及白名单打包/解包冒烟通过。最终解包Rust与独立归档Go正式程序各执行63项真实HTTP用例，数据库、材质文件、并发、跨语言令牌及重启对照通过。最终包配置/数据库检查、监听冲突、不可用数据库和正常退出通过。

Windows ZIP仅含程序、配置模板、README和部署说明，文档/模板与源码一致，SHA256为 1a8a989ab0314ad1605baa4c7387b86c73fb3d21532d1ba15296d384bf8050c9。证据见 rust-r5-verification.json、rust-r5-contract-comparison.json；日志位于 .local/rust-migration/logs/r5-*.log 和 test-output/windows-x64/logs/。

两个活动工作流均仅 workflow_dispatch，矩阵为Linux x64 musl、Linux ARM64 musl、Windows x64 MSVC。静态配置核验通过。Actions全部交由人工，Agent禁止运行；Linux编译、远端发布包、真实账号/客户端和生产项均为非阻塞 MANUAL/PENDING。
