# 当前任务交接

日期：2026-11-14。目标：按 RUST_MIGRATION_PLAN.md 完整实施 Rust 迁移，每阶段验证后提交推送，再继续。本地仅构建测试 Windows，Linux x64/ARM64 由 Actions 验证。

R0 自动验收通过：Go 1.26.4 Windows amd64 全量 go test -count=1 -p 1 -v ./... 通过，33 个顶层测试，无跳过；Windows 基线构建和 --check-config 通过。使用系统已有 MariaDB 的 aihmc_migration_test 隔离库，未安装新服务或写业务库。可选 PHP 反向执行未运行；真实账号、客户端及生产操作为非阻塞 MANUAL/PENDING。

证据在 docs/RUST_MIGRATION_PROGRESS.md、docs/rust-r0-verification.json、docs/rust-go-baseline-manifest.json。Go 源基线 cc458ab7d709323bdf4b9ca6f7ac1c857c4cfa09；本地 .local/rust-migration/ 存放忽略的合成配置、密钥、日志及基线程序，秘密不得提交。

分支 mc-auth 跟踪 origin/mc-auth；远端 https://github.com/AIH-MC/aihmc-service.git。R0 已提交推送 25eb204 并核对。R1 实现已提交推送 5c308b1 并核对。用户最新强制规则：Actions 完全交由人工，Agent 禁止运行，不得作为阶段或交付阻塞条件。build.yml 已改为仅 workflow_dispatch，当前修正待提交推送；自动 run 43579298110 已手动取消并确认 completed/cancelled，不再跟踪或重新运行。Linux/远端验收为非阻塞 MANUAL/PENDING。完成此配置修正提交后直接进入 R2。已有 rust.yml 是 aihmc-auth 分支的手动入口，保持原状。

已知验证问题：沙箱限制导致本机数据库及回环 HTTP 失败，正常权限复验消失；Go 包并行写同一隔离库发生跨包死锁，-p 1 后通过，包内并发用例保留。没有修改 Go 源码。后续同库对照应串行。

R1 新增 rust/ 配置/CLI/数据库入口/日志/根路径/正常退出及 scripts/ci.ps1，build.yml 增加当前源码三平台。Linux 按用户最新要求直接用 musl（x86_64 / aarch64），原生 runner + musl-tools + musl-gcc，readelf 检查架构和无动态依赖。本地只构建 Windows。

R1 fmt/Clippy/8项测试/release/打包解包/CLI冒烟通过；正式解包程序的真实隔离数据库检查、根路径GET/POST、端口冲突、数据库失败和 Ctrl+Break 优雅退出通过。证据见 docs/rust-r1-verification.json；本地运行验证脚本在 .local/rust-migration/r1-runtime-check.py。业务尚未迁移，阶段包不用于完整生产替换。R2 要完成六表及事务、查询超时和连接池空闲控制，R3 要补完整路由和HTTP读写时限，R4 材质和删除，随后归档交付。

禁止扩大范围、安装数据库服务器或重构旧 Go。所有构建测试打包前台 PowerShell 经 Tee-Object 保存 build.log 并立即检查退出码；无输出命令前清空日志，避免残留上一条输出。每阶段更新记录、审查差异、提交推送确认后才进入下一阶段。Qt 验证仅限真实 Windows 窗口。任务最终完成需再更新本手册。

可靠额度接口本轮返回周额度已用 46%，未达到交接阈值；未调用 Luna。
