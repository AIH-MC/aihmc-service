# 当前任务交接

日期：2026-11-14。目标：按 RUST_MIGRATION_PLAN.md 实施完整 Rust 迁移，每阶段本地自动验证后提交推送、核对远端，再继续下一阶段。任务仍在进行。

## 用户最新强制指示

- 本地只编译测试 Windows x64。
- Linux 直接用 musl：x86_64-unknown-linux-musl 和 aarch64-unknown-linux-musl。
- Actions 完全交由人工，Agent 禁止运行，不得作为阶段或交付阻塞条件。两个工作流均仅 workflow_dispatch，未人工执行项记非阻塞 MANUAL/PENDING。
- 自动触发的 run 43579298110 已手动取消并确认 completed/cancelled；不得重新运行或继续监控。保留原有 checkout aihmc-auth 分支的 rust.yml 手动入口，不把它的结果当作本项目 Rust 证据。

## 已完成与已验证

R0：Go 1.26.4 Windows amd64 全量 go test -count=1 -p 1 -v ./...，33 个顶层测试通过，无跳过；Windows 基线构建和 --check-config 通过。源基线 cc458ab7d709323bdf4b9ca6f7ac1c857c4cfa09，提交推送检查点 25eb204。

R1：rust/ 入口、配置、数据库连接、日志、根路径、正常退出和三平台编译配置，本地 fmt/Clippy/8项测试/release/发布包冒烟与真实启停通过；实现提交推送 5c308b1，取消自动触发及非阻塞规则修正提交推送 734f5b9，均已核对远端一致。Windows 程序实际依赖 VCRUNTIME140.dll，部署说明已在 rust/README.md 记录 x64 VC++ 运行库要求。

R2：六表、事务/期限、闲置连接、协议/序列化/RSA和UCenter完成，Windows15项全量测试及release/解包验证通过。提交4a66088442717db4a9fe973b833b587614555327已推送并核对。

R3：认证/会话/角色/元信息HTTP handler、代理信任和请求编码完成；新增保留 Go 语义的 HTTP/1 读头、整请求读、写和 keep-alive 空闲期限。Windows17项全量测试、45项Go/Rust真实HTTP及数据库结果对照、八并发首次登录、并发刷新、双向令牌读取、新Rust进程读取旧身份，以及真实TCP期限/超时数据库副作用对照全部通过。Black/rustfmt、Clippy、release、解包和正式二进制根路径/正常退出通过。当前R3待阶段提交推送，确认后进入R4。

## 文件与证据

主要R3修改：rust/src/auth.rs、httpapi.rs、server.rs；调整main使用新传输层、store事务传播业务错误；rust/examples/http_fixture.rs和tests/tools/http-fixture/main.go为独立对照进程；scripts/verify-contract.py、verify-timeouts.py为可复现HTTP/数据库/TCP验收脚本。另有依赖锁、格式和证据文档更新。生产Go未修改。

详细证据：docs/RUST_MIGRATION_PROGRESS.md、docs/rust-r0/r1/r2/r3-verification.json、docs/rust-r3-contract-comparison.json、docs/rust-r3-timeouts.json、docs/rust-go-baseline-manifest.json。忽略目录.local/rust-migration/保存配置、合成密钥、基线程序、Go桥和完整日志，不能提交秘密。test-output/windows-x64/保存阶段包和日志，rust/target/保存构建缓存。R3全量命令和参数在本聊天工具记录与脚本CLI中，可重现。

## 当前 Git 与下一步

分支mc-auth跟踪origin/mc-auth，origin https://github.com/AIH-MC/aihmc-service.git；当前已推送头4a66088，R3实现及证据待阶段提交。先完成R3提交推送核对，再实施R4材质头像、管理删除，并将实际材质依赖接入正式程序的完整HTTP入口（当前正式程序仅根路径，45项业务已由独立真实夹具进程验证）；随后R5归档Go、提升Rust到根、更新发布与部署；R6交付。R4至R6尚未完成。

## 环境与必须遵守

复用系统现有正在运行的 MariaDB，所有测试只用 aihmc_migration_test，不安装数据库服务器。AIHMC_TEST_CONFIG 指向 .local/rust-migration/config.toml；AIHMC_GO_PROTOCOL_BRIDGE 指向 .local/rust-migration/go-protocol-bridge.exe。桥的 Windows 构建入口为 ./tests/tools/protocol-bridge。R5 归档时还需保存新增测试桥并调整协议向量 include_str 路径。

沙箱内数据库/回环HTTP受限，需正常权限运行这些测试；不要误判为业务失败。Go 包间并行测试曾因共享隔离库死锁，使用 -p 1 后通过，包内并发用例保留。Rust 同库集成使用 --test-threads=1。

所有构建测试打包前台 PowerShell，经 Tee-Object 保存 build.log、立即检查退出码，失败读取日志。scripts/ci.ps1 内部已逐命令 Tee，直接调用，避免外层 Tee 和内部日志争用。SQLx 0.9 动态SQL须 QueryBuilder 或静态字符串，本项目已适配，不降级规避。

只改授权范围、保留已有修改、不强推、不擅自生产切换。Qt 只能真实 Windows 窗口验证。真实账号、客户端、生产、Actions和Linux均按实际状态记录非阻塞 MANUAL/PENDING。任务最终交付前再次更新本手册。

R3诊断记录：Python受控UCenter默认backlog=5无法承担八路登录连接，Go也稳定500；仅将测试服务器队列改64后Go/Rust均通过，未改变业务代码或降低并发。HTTP期限已独立对照，包括写期限不得提前取消事务及keep-alive不得套用读头期限。

最近可靠额度查询：周额度已用49%，未达到交接阈值，未使用Luna。
