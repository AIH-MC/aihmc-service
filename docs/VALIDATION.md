# Rust 迁移验收记录（历史阶段）

日期：2026-11-14；平台：Windows x64；Rust 1.95.0；Python 3.13.12；复用系统已运行的 MariaDB 10.6。

## 自动验证

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| Rust fmt / Clippy `-D warnings` | PASS | `test-output/logs/format.log`、`clippy.log` |
| Rust 单元测试 | PASS，8 项 | `test-output/logs/test.log` |
| Windows release 编译 | PASS | `test-output/logs/release.log` |
| Python 参考及数据库基线 | PASS，18 项 | `test-output/logs/windows-integration.log` |
| Rust 真实 HTTP / 现有实例隔离库集成 | PASS，6 项 | 同上 |
| 皮肤、缓存、头像集成 | PASS，4 项 | 同上；PNG 与 Pillow 完整像素对照 |
| 双服务差异回归 | PASS，2 项场景 | 同上；分别初始化数据库，覆盖全部 20 个业务路由与协议细节 |
| 归档 Python 独立启动及注册写库 | PASS，1 项 | 同上；复制归档到独立目录，运行原 `main.py` 与原配置加载器 |
| ZIP 解压启动、Token、默认头像资源 | PASS | `test-output/logs/package-resource-smoke.log` |
| 实际 ZIP 解压二进制的数据库/上游集成 | PASS，6 项 | `test-output/logs/unpacked-package-integration.log` |
| PowerShell 脚本语法、三平台矩阵及仅手动触发 | PASS，静态检查 | `.github/workflows/rust.yml` 只有 workflow_dispatch |
| 敏感文件排除及归档完整性 | PASS | ZIP 条目核对，Git ignore 检查，原源码/配置 12 文件迁移前后 SHA-256 一致 |

完整 Windows 检查命令为 `./scripts/check-windows.ps1`，前台执行并逐命令保存日志。31 项 Python 驱动测试全部通过，无跳过；其中 Rust 服务是真实独立进程，数据库为现有实例中的随机隔离库，运行结束清理。Rust 8 项单元测试单独计数。

最后对实际解压的 Windows ZIP 二进制补做资源冒烟和 6 项 HTTP/数据库集成，防止仅以源码编译结果代替发布包证据。

## 发布包

- 当时文件：`test-output/artifacts/aihmc-auth-windows-x64.zip`（后续 nickname 构建会覆盖此路径；最新结果见 `OFFLINE_NICKNAME.md`）。
- SHA-256：`9310218eae2156f59ac57c3919b13bbe34a5860219858decf98c0845d3e52946`。
- 内容：可执行文件、TOML 示例、默认皮肤与文档资源、keys/textures 目录标记、部署说明。
- 已核对不含 config.toml/config.yml、真实公钥/私钥、数据库凭据、缓存或本地索引。

## 实际迁移结果

- Rust 覆盖认证、账户、Figura、资料、皮肤、缓存和头像。配置正式改为 TOML，旧 YAML 可用转换脚本一次性转换，目标已存在则拒绝覆盖。
- 原 Python 业务源码、原依赖和 YAML 示例归档到 `legacy/python/`；源码未顺带修复。独立资源、原说明和参考环境清单随归档提供。
- 真实差异回归修正了 Rust 的公钥换行、404/405、HEAD、尾斜杠重定向、CORS 预检行为；文档入口保留。
- 旧版已确认问题仍按基线保留，详见 `PYTHON_BASELINE.md`。Python 原声明依赖在新环境的失败与参考环境差异明确记录，不宣称完成 Python 最新依赖升级。
- 本地私有配置已转换为忽略的 config.toml，原 YAML 未删除，凭据未输出或提交。

## 未运行项目

| 项目 | 状态 | 原因 |
| --- | --- | --- |
| Linux x64 编译、测试与包冒烟 | NOT RUN | 用户要求本地仅 Windows，Actions 不运行 |
| Linux ARM64 编译、测试与包冒烟 | NOT RUN | 同上 |
| GitHub Actions（含 Windows） | NOT RUN | 用户要求仅手动，当前未触发 |
| 实服验证、启动客户端认证（人工） | SKIPPED | 用户后续明确要求跳过；不作为阻塞条件 |
| 生产部署/流量切换 | NOT PERFORMED | 不在本轮授权范围 |

这些状态不记为 PASS，也不阻止已授权的实施与交付。本轮无剩余代码实施；维护者之后按需手动运行 Actions 和人工验收。

## 阶段提交

1. 阶段 1：`a78a388`，Python 行为基线。
2. 阶段 2：`df3da75`，Rust 基础、TOML 和手动三平台流水线。
3. 阶段 3：`f2da3b5`，认证、账户、Figura 和资料。
4. 阶段 4：`2c4e27a`，皮肤、缓存和头像。
5. 阶段 5：`e2811a1`，最终回归与 Python 独立归档；已推送 `origin/aihmc-auth`，远端完整提交 e2811a150f616e63f3f6bc971be55ee16220843d 核对一致。
