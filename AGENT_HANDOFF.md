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

R2：已实现六表访问、事务与期限、连接池空闲控制、协议/材质序列化/RSA、UCenter。Windows 15 项全量测试（include-ignored，test-threads=1）通过，包含真实 MariaDB 的 NULL、唯一约束、刷新、删除回滚、锁等待超时，以及 Go/Rust 双向签名；fmt/Clippy/release/解包CLI及真实启停通过。当前 R2 待阶段提交推送，确认后才能进入 R3。

## 文件与证据

主要 R2 修改：rust/Cargo.toml、Cargo.lock、src/database.rs、src/lib.rs、src/protocol.rs、src/store.rs、src/ucenter.rs、tests/protocol.rs、tests/store.rs、tests/ucenter.rs、rust/README.md，新增 tests/tools/protocol-bridge/main.go（仅合成资料测试桥，不改 Go 生产代码）。另有计划、实施记录、本手册及 R2 验证 JSON 更新。

详细证据：docs/RUST_MIGRATION_PROGRESS.md、docs/rust-r0-verification.json、docs/rust-r1-verification.json、docs/rust-r2-verification.json、docs/rust-go-baseline-manifest.json。忽略目录 .local/rust-migration/ 保存配置、合成密钥、基线程序、Go 桥和完整日志，不能提交秘密。test-output/windows-x64/ 保存阶段发布包和检查日志，rust/target/ 保存 Windows 构建缓存。

## 当前 Git 与下一步

分支 mc-auth 跟踪 origin/mc-auth，origin https://github.com/AIH-MC/aihmc-service.git；当前已推送头为 734f5b9，R2 工作区修改待提交。先完成 R2 diff/格式审查、提交、推送及远端核对，再实施 R3 认证/会话HTTP、完整路由与HTTP读写时限；R4 材质头像和管理删除；R5 Go归档/Rust提升根目录/发布脚本与部署说明；R6 交付。R3 至 R6 未完成，当前阶段程序仅开放根路径，不能宣称完整业务已迁移。

## 环境与必须遵守

复用系统现有正在运行的 MariaDB，所有测试只用 aihmc_migration_test，不安装数据库服务器。AIHMC_TEST_CONFIG 指向 .local/rust-migration/config.toml；AIHMC_GO_PROTOCOL_BRIDGE 指向 .local/rust-migration/go-protocol-bridge.exe。桥的 Windows 构建入口为 ./tests/tools/protocol-bridge。R5 归档时还需保存新增测试桥并调整协议向量 include_str 路径。

沙箱内数据库/回环HTTP受限，需正常权限运行这些测试；不要误判为业务失败。Go 包间并行测试曾因共享隔离库死锁，使用 -p 1 后通过，包内并发用例保留。Rust 同库集成使用 --test-threads=1。

所有构建测试打包前台 PowerShell，经 Tee-Object 保存 build.log、立即检查退出码，失败读取日志。scripts/ci.ps1 内部已逐命令 Tee，直接调用，避免外层 Tee 和内部日志争用。SQLx 0.9 动态SQL须 QueryBuilder 或静态字符串，本项目已适配，不降级规避。

只改授权范围、保留已有修改、不强推、不擅自生产切换。Qt 只能真实 Windows 窗口验证。真实账号、客户端、生产、Actions和Linux均按实际状态记录非阻塞 MANUAL/PENDING。任务最终交付前再次更新本手册。

最近可靠额度查询：周额度已用 48%，未达到交接阈值，未使用 Luna。
