# AIHMC Minecraft 认证服务（Rust）

Rust 单体服务承载原 Go 的外置登录、UCenter、会话验证、角色查询、材质、头像和管理删除接口。沿用现有 MySQL/MariaDB 六表、身份、令牌、RSA 密钥、TOML 配置和材质 URL，不自动改表或重新生成用户身份。

活动工程已位于仓库根目录。Go 完整快照在 [archive/go](archive/go/ARCHIVE.md)，旧 PHP 保留在 archive/php。迁移要求见 [计划](RUST_MIGRATION_PLAN.md)，阶段测试和提交证据见 [实施记录](docs/RUST_MIGRATION_PROGRESS.md)。

## 配置与运行

只提交 config.example.toml 模板。实际运行默认读取工作目录下的 config.toml，或通过 --config 指定；缺失时失败，不会使用模板兜底。继续使用已有配置值，各资源路径相对于配置文件目录解析。实际配置、秘密文件、私钥和运行材质不进入 Git 或发布包。

启动检查配置、秘密文件、公私钥匹配、可写材质目录及数据库连接。--check-config 完成检查后退出；--help 不需要运行配置或数据库。Windows 运行示例：

```powershell
& ./aihmc-api.exe --config 'C:/service/config.toml' --check-config 2>&1 | Tee-Object -FilePath 'build.log'
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath 'build.log'
    exit $buildExitCode
}
```

默认监听回环地址，由现有网关提供 TLS 及 /textures/ 静态资源；网关必须覆盖 X-Real-IP。仅当 TCP 对端为回环地址时读取该头，hasJoined 不新增 IP 校验。Windows 使用 Ctrl+C/Ctrl+Break 正常停止；Linux 使用 SIGINT/SIGTERM。配置通过重启生效。

Windows 版本需要 x64 VC++ 运行库（VCRUNTIME140.dll）。Linux 两个目标按 musl 静态链接配置，HTTPS 仍需目标系统的受信任 CA 证书。实际部署和回退见 [部署手册](docs/DEPLOYMENT.md)。

## 本地 Windows 构建

使用 rust-toolchain.toml 固定的 Rust 1.95.0、Windows MSVC 工具链和提交的 Cargo.lock。在仓库根目录执行：

```powershell
& ./scripts/ci.ps1 -Target x86_64-pc-windows-msvc -Platform windows-x64
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath 'build.log'
    exit $buildExitCode
}
```

脚本内部逐条前台执行 fmt、Clippy、测试、release、打包、解包及 CLI 冒烟，均通过 Tee-Object 记录 build.log 并立即检查状态。不要再用外层 Tee 写同一个日志文件。产物为 test-output/windows-x64/artifacts/aihmc-service-windows-x64.zip 及对应 .sha256；解包后包含 aihmc-api.exe、配置模板、README.md 和 DEPLOYMENT.md。

## 自动验证

普通 Cargo 测试会明确忽略 3 项需要现有数据库或 Go 校验桥的集成用例。全量验证须设置 AIHMC_TEST_CONFIG 为合成测试配置的绝对路径（database.name 必须为 aihmc_migration_test），并设置 AIHMC_GO_PROTOCOL_BRIDGE 指向从 archive/go/tests/tools/protocol-bridge 构建的 Windows 校验程序，然后执行：

```powershell
& cargo test --locked --target x86_64-pc-windows-msvc -- --include-ignored --test-threads=1 2>&1 | Tee-Object -FilePath 'build.log'
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath 'build.log'
    exit $buildExitCode
}
```

只复用系统已有 MySQL/MariaDB，测试写入隔离库，不安装额外数据库服务器。Go 测试包之间使用 -p 1，Rust 共享库用例使用 --test-threads=1。

scripts/verify-contract.py --production 可对原 Go 与实际 Rust 程序执行真实 HTTP、数据库及材质文件对照，覆盖登录/刷新/进服、并发、重启、头像像素和删除失败回滚；需要 Python、Pillow、已有 MySQL 客户端和显式测试路径。scripts/verify-timeouts.py 验证真实 TCP 的读头、整请求、空闲及写期限。详细调用参数使用脚本 --help 查看，验收证据见实施记录。

## 手动 Actions

Agent 不运行 Actions。人工在 mc-auth 分支选择 **Build Rust (manual)**（.github/workflows/build.yml）执行。提交和 PR 不会自动构建。另一个 Rust manual build 是原有 aihmc-auth 分支入口，不能代替本实现的验收。

| 平台 | Rust target | 发布文件 |
| --- | --- | --- |
| Linux x64 | x86_64-unknown-linux-musl | aihmc-service-linux-x64.tar.gz |
| Linux ARM64 | aarch64-unknown-linux-musl | aihmc-service-linux-arm64.tar.gz |
| Windows x64 | x86_64-pc-windows-msvc | aihmc-service-windows-x64.zip |

工作流复用原生 runner、musl-gcc 和固定工具链，执行同一 CI 脚本；Linux 还检查 ELF 架构及不存在 INTERP/NEEDED 动态依赖，正式包与 SHA256 分别上传。Linux/远端 Actions、真实账号、客户端和生产操作未执行时记为非阻塞 MANUAL/PENDING，不因这些人工项暂停代码交付。
