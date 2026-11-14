# Rust 迁移实施记录

日期：2026-11-14。实施依据：`../RUST_MIGRATION_PLAN.md`。

用户授权按 R0 至 R6 顺序实施，每阶段验证后提交推送，再继续下一阶段。本地仅构建和测试 Windows x64，Linux x64/ARM64 由 Actions 执行。

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

生产、真实账号和客户端验收：MANUAL/PENDING，非阻塞。R1 至 R6 尚未完成。
