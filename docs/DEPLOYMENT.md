# Rust 部署（迁移实施中）

阶段 2 的包仅提供基础 HTTP 与 Token 校验，尚不能替换完整 Python 后端。

解压对应架构包，复制 `config.example.toml` 为 `config.toml`，填写现有数据库连接、访问 Token 及密钥路径。在包目录运行 `aihmc-auth.exe`（Windows）或 `./aihmc-auth`（Linux）。可用 `--config <绝对或相对路径>` 指定配置。

资源相对路径以配置文件所在目录为基准，保留 `assets/alex.png`、`keys/`、`textures/`。不自动生成/替换密钥，不自动创建或清空数据库。Rust 使用 TOML，不自动回退到 YAML；原 YAML 的顶层标量必须放在 TOML 各表定义之前，servers 转为 `[[servers]]`。

`server.debug` 保留配置兼容，但 Rust release 不提供 Python 的代码热重载。`server.timeout` 设置上游请求超时（秒）。

本地只验证 `x86_64-pc-windows-msvc`。Actions 仅 workflow_dispatch，由用户手动运行，开发提交不自动触发。Linux 包使用 Ubuntu 24.04 原生构建，最低实际系统/动态库要求需等待手动构建结果，不宣称支持旧 glibc 或所有发行版。

生产切换须先备份配置、密钥、数据库和缓存，获授权后切换入口。无表结构迁移；回滚恢复原 Python 入口及已备份配置，不清理业务数据。原 Python 在最终迁移完成后归档至 `legacy/python/`。
