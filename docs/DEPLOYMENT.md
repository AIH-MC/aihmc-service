# Rust 部署

Rust 已覆盖原业务路由。Windows 自动验收包括真实 HTTP、现有 MariaDB 隔离库、Python 对照及 PNG 像素回归；Linux 构建和真实插件/上游账号验收状态单独记录。

解压对应架构包，复制 `config.example.toml` 为 `config.toml`，填写现有数据库连接、访问 Token 及密钥路径。在包目录运行 `aihmc-auth.exe`（Windows）或 `./aihmc-auth`（Linux）。可用 `--config <绝对或相对路径>` 指定配置。

资源相对路径以配置文件所在目录为基准，保留 `assets/alex.png`、`keys/`、`textures/`。不自动生成/替换密钥，不自动创建或清空数据库。Rust 使用 TOML，不自动回退到 YAML；原 YAML 的顶层标量必须放在 TOML 各表定义之前，servers 转为 `[[servers]]`。

`server.debug` 保留配置兼容，但 Rust release 不提供 Python 的代码热重载。`server.timeout` 设置上游请求超时（秒）。

本地只验证 `x86_64-pc-windows-msvc`。Actions 仅 workflow_dispatch，由用户手动运行，开发提交不自动触发。Linux 包使用 Ubuntu 24.04 原生构建，最低实际系统/动态库要求需等待手动构建结果，不宣称支持旧 glibc 或所有发行版。

生产切换须先备份配置、密钥、数据库和缓存，获授权后切换入口。无表结构迁移；回滚恢复 `legacy/python/main.py`、原 YAML 配置和已备份密钥，不清理业务数据。Python 归档包含独立资源及运行说明。

旧配置转换：使用 `scripts/convert_config.py` 一次性转换 YAML 为 TOML；既有目标配置不会覆盖，旧 YAML 不删除。不要把真实配置、密钥、缓存或数据库备份提交到仓库。

API 保留原 `/docs`、`/redoc`、`/openapi.json` 及 Swagger OAuth 重定向入口；OpenAPI 为从 Python 基线导出的静态兼容契约。Swagger/Redoc 页面和原版一样从外部 CDN 加载前端资源。

资料关联写库成功仍返回 400、带 IP 的 hasJoined 查询拼接、Mojang CSL 不下载材质仍按基线保留。后续 nickname 功能已替换冲突名称分配：在线名称优先、离线随机名持久化，UUID 不变；不再使用会覆盖其他账户映射或无限重试的旧改名路径。见 `docs/OFFLINE_NICKNAME.md`。

实服验证和启动客户端认证均为人工验收／SKIPPED（用户要求跳过）；生产流量未操作。Actions 按用户要求未运行，Linux x64/ARM64 为 NOT RUN，不使用 Windows 结果替代。
