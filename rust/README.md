# Rust 迁移阶段构建

当前已完成 R1 至 R4 的入口、数据、协议、认证会话、材质头像和管理删除，正式程序已接入完整业务路由。尚待 R5 将 Rust 提升到根目录并归档 Go，以及 R6 完成交付记录。生产操作和真实账号验收按迁移计划人工交接。

二进制名称为 `aihmc-api`，Windows 为 `aihmc-api.exe`。支持 `--help`、`--config <path>`、`--check-config`。实际配置使用已有 TOML 格式，相对资源路径相对于配置文件目录；模板不能作为运行配置。启动检查密钥、秘密文件、材质目录和现有数据库，程序不建库、不改表。

从仓库根目录运行 `scripts/ci.ps1 -Target x86_64-pc-windows-msvc -Platform windows-x64`，即可前台执行格式检查、Clippy、测试、release 构建、打包、解包与 CLI 冒烟，各命令内部均记录 build.log 并检查退出状态。阶段产物位于 `test-output/windows-x64/artifacts/`。Linux 两个目标由 GitHub Actions 原生 runner 验证。

Windows 使用 Ctrl+C/Ctrl+Break、Linux 使用 SIGINT/SIGTERM 正常关闭。Windows 需要 x64 VC++ 运行库（VCRUNTIME140.dll）。Linux x64/ARM64 均使用 musl target，流水线配置包含 ELF 架构与静态链接检查；Actions 由人工运行，不由 Agent 启动。完整部署和接口验收以根目录 `RUST_MIGRATION_PLAN.md` 及 `docs/RUST_MIGRATION_PROGRESS.md` 为准。包内 DEPLOYMENT.md 为现有 Go 部署参考，Rust 最终版本会在归档阶段更新。
