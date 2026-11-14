# 最终任务交接

更新：2026-11-14。当前任务为按计划将Go服务迁移到Rust、独立归档Go、交付Linux x64/ARM64 musl及Windows x64手动编译配置。本地只编译验证Windows。R0—R6自主实施及自动验收完成，无剩余实施阶段；人工项目不阻塞交付。

## 当前布局与证据

Rust正式工程位于根目录，所有受支持HTTP路径已组装到正式入口；Cargo.lock及Rust1.95.0固定工具链已提交。Go独立归档archive/go，60个文件工作区及Git暂存blob的SHA256均与ARCHIVE_MANIFEST.json匹配。archive/php未修改。没有业务schema升级；继续复用配置、六表、身份、令牌、密钥和材质。

Windows Rust全量27测试（含数据库/跨语言）、独立Go归档33顶层测试和构建、fmt、Clippy -D warnings、release、白名单打包和解包冒烟均PASS。最终Rust解包程序与独立归档Go各63真实HTTP请求对照、数据库/文件一致性、删除故障回滚、并发、双向令牌和重启PASS。十组头像像素及真实TCP期限验证PASS。最终包配置/现有数据库、监听冲突、不可用数据库和正常退出PASS。没有未解决的自动验收失败。

Windows包：test-output/windows-x64/artifacts/aihmc-service-windows-x64.zip；SHA256：1a8a989ab0314ad1605baa4c7387b86c73fb3d21532d1ba15296d384bf8050c9。仅含exe、模板、README、DEPLOYMENT。真实配置、秘密及测试日志在被忽略的.local中，未提交或打包。

计划：RUST_MIGRATION_PLAN.md；阶段记录和提交表：docs/RUST_MIGRATION_PROGRESS.md；最终证据：docs/rust-r5-verification.json、docs/rust-r5-contract-comparison.json、docs/rust-r6-delivery.json。日志：.local/rust-migration/logs及test-output/windows-x64/logs。部署和回退：docs/DEPLOYMENT.md。

## Git检查点

分支mc-auth跟踪origin/mc-auth，远端https://github.com/AIH-MC/aihmc-service.git。R0至R5均已逐阶段提交推送，最近已核对远端为bc8b52632143cf158133a45de542e94a941d4ddb。R6为当前最终文档检查点；本手册随R6提交推送，最终哈希和推送结果由执行者交付时核对，使用Git历史查询。R6只改计划、实施记录、交付索引和本手册，不改变已验证程序或发布包。提交前工作区仅这四份R6文档，提交后预期干净。

## 人工待验（非阻塞）

Actions、Linux x64/ARM64 musl实际构建/运行、真实UCenter账号/启动器/Minecraft客户端、生产网关/切换/观察/回退均MANUAL/PENDING，未记PASS。没有已执行的生产操作。人工如需继续，按部署手册取得并核对包，在mc-auth选择Build Rust (manual)，记录run/提交/架构/SHA256及目标机结果。原Rust manual build检出aihmc-auth分支，不能代替本实现验收。没有剩余Agent实施任务，不为人工等待轮询或启动新阶段。

## 继续适用的约束

Actions全部人工，Agent禁止运行或作为阻塞条件；根工作流只有workflow_dispatch，提交/PR不触发。意外run43579298110已取消且终态确认，不重跑或监控。Linux必须musl，本地只编译Windows。任何构建/测试均前台PowerShell经Tee-Object写build.log并立即检查退出码；ci.ps1内部已处理，不外层写同一日志。MySQL只复用系统既有实例和隔离aihmc_migration_test，禁止另装服务端。按阶段提交推送确认、不强推、不扩大范围、保留已有工作区修改、禁止Qt offscreen、不泄露实际配置秘密。
