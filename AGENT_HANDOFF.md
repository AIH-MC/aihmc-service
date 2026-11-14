# Agent Handoff

- 目标：按 RUST_MIGRATION_PLAN.md 完成 Rust 迁移，分阶段提交推送。
- 已推送：阶段 1 a78a388；阶段 2 df3da75；阶段 3 f2da3b5。
- 阶段 4 完成待检查点：CSL、材质缓存/文件、头像和默认资源。
- 验证：Windows Rust 1.95 fmt/Clippy、8 单元、release 包解压冒烟、4 项真实 HTTP 皮肤集成通过；包括与 Pillow 完整像素对照、13 倍尺寸、两种 UUID、缓存命中和远程失败回退。
- 日志：test-output/logs/integration-stage4.log，test.log、release.log、smoke.log；所有服务/隔离库已清理。
- 下一步阶段 5：完整路由回归、文档入口、生产部署说明、Python 归档 legacy/python/、最终 Windows 包验证。
- 用户补充：Rust 用 TOML，本地仅 Windows x64，Actions 仅手动且不得运行，原 Python 完成后归档。
- 当前 aihmc-auth，origin AIH-MC/aihmc-service；本阶段提交推送结果以实际 Git 为准。
- 配置凭据只在忽略的 config.yml；测试生成 config.toml 结束清理，绝不提交/输出凭据。
- 已证实旧行为按 docs/PYTHON_BASELINE.md 保留，包括 Mojang 材质下载分支不匹配；无关修复另行决定。
- 未验证：Linux/Actions NOT RUN，真实插件/上游账号 MANUAL/PENDING；不为人工待验停止自主交付。
- 保持范围、复用现有 MySQL、前台 PowerShell 日志和阶段检查点要求。
