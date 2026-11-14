# AIHMC Auth Service（Rust）

[AIH-MC Auth 插件](https://github.com/AIH-MC/aihmc-auth)的配套认证后端。当前服务源码为 Rust，原 Python 版本独立归档于 [legacy/python](legacy/python/README.md)。

保留多源 Yggdrasil 认证、离线账户、Figura、资料关联/改名、CSL、材质缓存及 PNG 头像接口。迁移及已保留的旧行为见 [迁移计划](RUST_MIGRATION_PLAN.md)和 [Python 基线](docs/PYTHON_BASELINE.md)。

离线账户支持 `/nickname <昵称>`：登录器和登录命令继续使用原账户名，重连后以昵称进服，UUID 不变。与正版/外置名称冲突时只给离线账户分配持久随机名。需同时更新插件，详见 [昵称与语言变量说明](docs/OFFLINE_NICKNAME.md)。

## 运行

1. 解压对应平台发布包，复制 `config.example.toml` 为 `config.toml`。
2. 填写现有 MySQL/MariaDB 连接、访问 Token、公钥路径和认证来源。
3. 在包目录执行 `./aihmc-auth`，Windows 执行 `./aihmc-auth.exe`。也可指定 `--config <路径>`。

相对资源路径以配置文件目录为基准。保留 assets、keys、textures；不自动初始化/清空数据库或生成密钥。文档入口为 `/docs`、`/redoc`、`/openapi.json`。

旧 YAML 可用 Python 3.11+ 与 PyYAML 一次性转换：`python scripts/convert_config.py config.yml config.toml`。转换保留值、不输出凭据、不覆盖目标文件。正常运行 Rust 不需要 Python，也不会回退读取 YAML。

## 编译与验证

工具链固定为 Rust 1.95.0。Windows 需要 MSVC C++ 构建工具。本地仅验证 Windows x64：

```powershell
& ./scripts/ci.ps1 -Target x86_64-pc-windows-msvc -Platform windows-x64
```

脚本逐条前台执行格式、Clippy、测试、release、打包及解包 HTTP 冒烟，保存终端日志。Windows 完整回归（另需 `.venv` 安装 `requirements-test.txt` 和现有数据库隔离库权限）：

```powershell
& ./scripts/check-windows.ps1
```

测试读取忽略的 `config.toml`，也可设置 `AIHMC_TEST_CONFIG` 指向连接配置；只使用随机 `aihmc_migration_test_*` 库并清理，不访问业务表。

## 手动 Actions

`.github/workflows/rust.yml` 只有 `workflow_dispatch`。用户手动运行后，原生编译测试 Linux x64、Linux ARM64、Windows x64，上传归档、SHA-256 和日志。提交和推送不自动触发。本次迁移按要求未执行 Actions，Linux 结果仍为 NOT RUN。

工作流位于 `aihmc-auth` 分支；GitHub 的手动运行入口受默认分支工作流可见性规则限制。需要在仓库中提供该手动入口后由维护者运行，本次不修改默认分支、不代为触发。

部署、备份、兼容限制和回滚见 [部署说明](docs/DEPLOYMENT.md)。
