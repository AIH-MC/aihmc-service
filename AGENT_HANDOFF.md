# Agent Handoff

- 任务：按 RUST_MIGRATION_PLAN.md 迁移 Rust，阶段完成提交推送。
- 当前阶段：阶段 1 完成，检查点提交推送后开始阶段 2。
- 用户补充：Rust 配置改 TOML；完成迁移后原 Python 完整归档 legacy/python/；本地仅 Windows x64；Actions 仅手动且不要运行。
- 完成：20 路由响应基线、算法/PNG/来源 URL、真实 Uvicorn 回环、现有 MariaDB 隔离库测试；Python 业务源码无修改。
- 验证：Windows Python 3.13.12，18 项 unittest 通过；black 格式化新增测试；日志 build.log（忽略）。
- 参考环境差异：python_bcrypt bytes API 和 PyMySQL 1.2.4 编码接口已复现失败；单独固定参考环境，详见 docs/PYTHON_BASELINE.md。不声称原声明依赖全功能可用。
- 配置：用户已提供忽略的 config.yml，可连接现有 MariaDB；不得输出/提交连接凭据；测试随机隔离库已清理。
- 分支：aihmc-auth，origin 为 AIH-MC/aihmc-service；阶段 1 正在提交推送，命令结果为准。
- 后续：阶段 2 Cargo/TOML/基础 HTTP/算法/数据库池/手动 CI，再按计划完成阶段 3—5。
- 未验证：Rust、Linux、真实插件/上游（MANUAL/PENDING）；Actions NOT RUN（用户指示）。
- 约束：按阶段顺序；前台 PowerShell Tee-Object 构建测试日志；复用 MySQL；不扩范围；不运行 Actions。
