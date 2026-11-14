# Agent Handoff

- 目标：按 RUST_MIGRATION_PLAN.md 迁移 Rust，每阶段提交推送。
- 已推送：阶段 1 a78a388；阶段 2 df3da75。
- 阶段 3 已完成：多源认证、账户、Figura、资料；准备阶段检查点提交推送。
- 验证：Windows Rust 1.95；fmt、Clippy、6 单元、release 打包解压 HTTP 冒烟、6 release 真实 HTTP/MariaDB 隔离库集成通过。
- 测试证据：test-output/logs/integration-stage3.log、test.log、release.log、smoke.log；测试数据库与服务已清理。
- 兼容：新旧 bcrypt 双向验密，来源顺序/禁用、签名、资料关联、会话；原 HTTP 400 关联副作用和带 IP 查询拼接等按基线保留，参见 docs/PYTHON_BASELINE.md。
- 下一步：阶段 4 皮肤/材质/头像，随后阶段 5 完整回归、文档端点、Python 归档和部署文档。
- 用户要求：Rust TOML；最终原 Python 归档 legacy/python/；本地仅 Windows x64；Actions 仅手动且不运行。
- 分支 aihmc-auth，origin AIH-MC/aihmc-service；提交推送结果以实际 Git 为准。
- 本地忽略的 config.yml 可连接现有 MariaDB，绝不输出/提交凭据；config.toml 同样忽略。
- 未验证：Linux/Actions（NOT RUN）、真实插件/账号（MANUAL/PENDING），不作为等待阻塞。
- 不扩大业务范围；数据库错误/无界重试等原行为未顺带改造；所有构建测试前台 Tee-Object 日志。
