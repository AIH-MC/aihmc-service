# Agent Handoff

- 目标：按 RUST_MIGRATION_PLAN.md 迁移 Rust，每阶段完成提交推送。
- 阶段 1 已推送：a78a388；Windows Python 参考测试 18 项通过，包含真实 HTTP 与现有 MariaDB 隔离库。
- 阶段 2 已完成：Cargo、TOML、基础 HTTP/Token、算法、共享客户端和池、三平台手动工作流、打包冒烟脚本。
- 验证：Rust 1.95.0 Windows x64；fmt、Clippy -D warnings、5 测试、release 编译和 ZIP 解压真实 HTTP 冒烟全部通过；日志 test-output/logs/。
- 依赖：axum 0.8.9、sqlx 0.9.0、reqwest 0.13.5、tokio 1.54.1、toml 1.2.1；Cargo.lock 固定解析。
- CI：仅 workflow_dispatch，三个原生 runner；工作流/PowerShell 静态检查通过，未执行 Actions。
- 当前分支 aihmc-auth；阶段 2 记录更新后提交推送，实际结果见 Git。
- 下一步：阶段 3 迁移认证/账户/Figura/资料并执行 Windows 隔离库 HTTP 集成测试。
- 用户要求：配置改 TOML；完成后原 Python 归档 legacy/python/；本地仅 Windows x64；禁止运行 Actions。
- 本地 config.yml 已由用户配置且忽略，不能提交/输出凭据。Rust config.toml 同样忽略。
- 已证实基线差异：见 docs/PYTHON_BASELINE.md；Python 参考环境与原依赖差异已记录，不冒充原环境完全通过。
- 未验证：完整 Rust 业务、Linux、真实插件/上游；人工项 MANUAL/PENDING，Actions NOT RUN。
- 保留现有 MySQL 实例，只使用随机测试库；遵守前台构建测试日志及阶段检查点，不扩大业务范围。
