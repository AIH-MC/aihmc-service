# 当前任务交接

更新：2026-11-14。目标仍在进行：完整 Rust 迁移、逐阶段验证后提交推送；本地仅验证 Windows x64。

## 不得违反的用户更正

Linux 使用 x86_64-unknown-linux-musl / aarch64-unknown-linux-musl。Actions 全部交由人工，Agent 禁止运行，不得作为阶段或交付阻塞条件；两个活动工作流仅 workflow_dispatch。自动 run 43579298110 已取消且终态已确认，不重新运行或监控。Actions/Linux、真实账号、客户端和生产项均为非阻塞 MANUAL/PENDING。

## 已完成及检查点

- R0 基线：Go 33 个顶层测试、Windows 构建及配置检查通过。已推送 25eb204。
- R1 入口/配置/生命周期和矩阵：Windows 验证通过，已推送 5c308b1；仅手动触发规则修正已推送 734f5b9。
- R2 六表/事务/协议/UCenter：15 项全量及发布验证通过，已推送 4a66088442717db4a9fe973b833b587614555327。
- R3 认证会话/HTTP 时限：17 项全量、45 项真实 handler 对照、并发、令牌互认、重启和 TCP 期限通过，已推送 5ff72c38f54699da3949dcccd807b344eadc07b1。
- R4 材质头像/删除及正式程序组装：27 项 Windows 全量、十组 GD 像素、7 项材质用例通过；原 Go 与最终 Rust release 解包程序各 63 项真实请求对照、数据库和材质文件一致，删除中途 SQL 故障回滚、并发、双向令牌、重启及正常退出通过。当前 R4 待阶段提交推送，确认后进入 R5。

分支 mc-auth 跟踪 origin/mc-auth，origin https://github.com/AIH-MC/aihmc-service.git。已推送头为 5ff72c3，当前 R4 修改尚未提交。工作区包含 R4 Rust 图像/材质/路由/main、测试、脚本和证据文档；没有改 Go 生产代码。

## 下一步

完成 R4 检查点后执行 R5：将 Go 独立归档到 archive/go/，Rust 从 rust/ 提升根目录；修正 Cargo 测试 fixture 路径和 CI 工作目录，移除当前实现的 Go 活动 job（保留另一个 checkout aihmc-auth 的手动 rust.yml），更新 Rust README/部署与回退说明。再从最终根目录验证 Windows、独立归档 Go、正式包 63 项对照、文件清单和 SHA256，提交推送；R6 完成交付审计和本手册最终状态。R5/R6 尚未实施。

Go 源基线 cc458ab7d709323bdf4b9ca6f7ac1c857c4cfa09。docs/rust-go-baseline-manifest.json 列出 58 个原文件及工作区 SHA256。Go 生产源码未变；README 和活动 build.yml 已随迁移修改，归档时从基线 Git blob 恢复原件并按清单核验 CRLF/LF。另保存本次新增的 tests/tools/protocol-bridge/ 和 tests/tools/http-fixture/。旧 PHP 归档保持原样。Go 测试的 Root() 依赖 GO_MIGRATION_PLAN.md，archive/go 内须保留计划、模板、docs 和 tests 样本。归档不得含实际配置、秘密、私钥、运行材质或构建缓存。

## 验证与环境

证据：docs/RUST_MIGRATION_PROGRESS.md；docs/rust-r0-verification.json、rust-r1-verification.json、rust-r2-verification.json、rust-r3-verification.json、rust-r4-verification.json；docs/rust-r3-contract-comparison.json、rust-r3-timeouts.json、rust-r4-contract-comparison.json。完整日志与配置在忽略目录 .local/rust-migration/，产物在 test-output/windows-x64/artifacts/。

系统已有 MariaDB 正在运行，测试库固定 aihmc_migration_test。AIHMC_TEST_CONFIG=.local/rust-migration/config.toml，AIHMC_GO_PROTOCOL_BRIDGE=.local/rust-migration/go-protocol-bridge.exe（命令用绝对路径）。Python 使用相邻 ../aihmc-auth-backend/.venv/Scripts/python.exe，已有 Black 和 Pillow；MySQL 客户端为 D:/Program Files/MariaDB 10.6/bin/mysql.exe。

全量 Rust：cargo test --locked --target x86_64-pc-windows-msvc -- --include-ignored --test-threads=1。普通 CI 显式 ignored 3 项需本地数据库/Go 桥的测试，本轮全量已执行，不能只凭普通 cargo test 声称全部验证。

完整正式程序对照：scripts/verify-contract.py --production --config <测试配置> --go <Go正式程序> --rust <Rust正式包解包程序> --bridge <Go协议桥> --mysql <客户端> --output <证据JSON>。R4 使用 .local/rust-migration/aihmc-api-go.exe 对 test-output/windows-x64/unpacked/aihmc-api.exe。--production 需要 Pillow，对照实际 PNG 像素；故障触发器仅在隔离库创建并于 finally 删除。

scripts/ci.ps1 内部逐命令前台 Tee，直接调用，避免外层 Tee 与内部 build.log 争用。当前默认 ProjectDirectory=rust，R5 改为根目录。所有编译测试打包都前台 PowerShell、Tee-Object、立即检查退出码，失败读取日志。数据库/回环连接在沙箱中受限，需正常权限；Go 包间测试用 -p 1，Rust 同库测试用 --test-threads=1。

## 诊断及待人工项

受控 Python UCenter 的默认 backlog=5 曾使 Go/Rust 并发失败，测试队列改 64 后保留八请求压力通过，无生产修改。HTTP/1 的读头、整请求、keep-alive、写期限及超时数据库副作用已实际对照；不能用 Hyper 单个读头计时器替代空闲期限，也不能在无数据 TCP flush 中提前取消处理器。正式程序现已接入完整业务，不再只开放根路径。

Windows 可执行文件需要 x64 VC++ 运行库 VCRUNTIME140.dll，最终部署说明须保留要求。Linux musl 及 Actions 不运行，记 MANUAL/PENDING。生产未切换；不把受控上游当真实用户联调。

保持范围、保留他人修改、不强推、不安装数据库服务器、不执行生产删除或切换。Qt 验证仅真实 Windows 窗口。最近可靠额度：周额度已用 49%，未触发交接阈值，未使用 Luna。
