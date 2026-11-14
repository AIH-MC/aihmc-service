# AIHMC Minecraft 认证服务

使用 Go 承载原 PHP 的 Minecraft 外置登录、UCenter 用户对接、会话验证、材质和头像接口。复用现有 MySQL/MariaDB 表、用户身份、令牌和 RSA 密钥，应用配置使用 TOML。

Rust 迁移已启动，计划见 [Rust 迁移计划](RUST_MIGRATION_PLAN.md)，阶段验证和提交记录见 [Rust 实施记录](docs/RUST_MIGRATION_PROGRESS.md)。迁移期间 `rust/` 中的构建为阶段验证产物，根目录 Go 仍为完整服务；本地仅验证 Windows，Linux x64/ARM64 在 Actions 验证。

当前迁移进度、验收证据与待完成事项见 [迁移计划](GO_MIGRATION_PLAN.md) 和 [实施记录](docs/MIGRATION_PROGRESS.md)。需要账号的真实用户和客户端联调按用户要求作为非阻塞人工验收 `MANUAL/PENDING`；M6 生产切换已按用户确认转为非阻塞人工交接。Go 模块位于仓库根目录，旧 PHP 源码保存在 `archive/php/`。

## 配置与运行

仓库只提交 `config.example.toml` 模板。实际运行默认读取当前工作目录的 `config.toml`，也可用 `--config <路径>` 指定文件；不会自动使用模板。数据库密码、UCenter secret、管理密钥和 RSA 私钥都不进入 Git。

原 `config.php` 已迁移为本地被忽略的 `config.toml` 与 `secrets/` 文件。部署时逐项核对真实环境值，不用模板默认值覆盖原配置。相对密钥、秘密文件和材质目录路径均相对于配置文件目录解析。

启动会检查配置字段、时长、文件权限、公私钥匹配、材质目录和数据库连接。可先运行 `--check-config` 完成这些检查再退出；它不证明真实用户能够登录或进服。配置修改通过重启生效。

只连接系统已有 MySQL 或 MariaDB，不另外安装数据库服务、便携版或容器实例。Go 侧使用 `database/sql` 与 `go-sql-driver/mysql`，普通启动不会创建数据库或执行 schema 变更。生产升级不得执行原始 `db.sql` 的删除语句。

## 构建 Linux x86_64

在包含 `go.mod` 的仓库根目录执行。发布目标仅为 Linux amd64，禁用 CGO；构建不需要 MySQL 客户端库。

```powershell
$env:GOOS = 'linux'
$env:GOARCH = 'amd64'
$env:CGO_ENABLED = '0'
& go build -trimpath -ldflags '-s -w' -o ./dist/aihmc-api-linux-amd64 ./cmd/aihmc-api 2>&1 | Tee-Object -FilePath 'build.log'
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath 'build.log'
    exit $buildExitCode
}
```

Linux 二进制不能直接在 Windows 中运行。交叉编译和 ELF 架构检查已通过，目标 Linux 环境的实际运行仍待验证。统一 GitHub Actions 位于 `.github/workflows/build.yml`，在 `mc-auth` 推送、面向该分支的 PR 或手动触发时，仅构建 Linux amd64。产物 `aihmc-service-linux-amd64.tar.gz` 包含可执行文件、配置模板和说明，另附 SHA256；保留可执行权限，实际运行配置和秘密不进入产物。流水线只使用 Go 工具链，不安装数据库或 PHP。

## 验证

在新的 PowerShell 会话中进入 Go 模块目录，设置实际运行配置的路径，再运行测试。不要沿用交叉编译会话的 GOOS/GOARCH 设置运行本机测试。

```powershell
$env:AIHMC_TEST_CONFIG = (Resolve-Path './config.toml').Path
& go test -count=1 ./... 2>&1 | Tee-Object -FilePath 'build.log'
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath 'build.log'
    exit $buildExitCode
}
```

数据库测试使用配置中的已有服务器连接信息，强制选择独立的 `aihmc_migration_test`，按 `tests/schema.sql` 创建测试表，并清理各用例自己的合成记录；测试连接账号需要该测试库的建库、建表和读写权限。不会向配置原指向的业务库写入测试数据，也不需要额外数据库实例。

没有设置 `AIHMC_TEST_CONFIG` 时，数据库和完整 HTTP 集成用例会跳过，不能据此宣布全量验收通过。PHP 不是服务运行或构建依赖；需要额外复验原 PHP 对 Go 令牌的识别时，可设置 `AIHMC_PHP_EXE` 与指向隔离旧项目副本的 `AIHMC_LEGACY_ROOT`。

固定向量与对照结果位于 `docs/legacy-protocol-vectors.json`、`docs/contract-comparison.json` 和 `tests/testdata/`。39 项实际 PHP/Go HTTP 用例经明确的随机值归一化后通过；新材质 Base64 字符串另有固定输入的逐字节比较，十组头像参考逐像素比较。

## 部署与回滚

见 [部署和回滚手册](docs/DEPLOYMENT.md)。默认由同机网关提供 TLS 和 `/textures/` 静态文件，Go 监听回环地址。网关必须覆盖 `X-Real-IP`；Go 只在 TCP 对端为回环地址时读取这个头，直接远程连接不能通过该头改变记录 IP。hasJoined 继续保持旧实现不校验 IP 的行为。

交付分支固定为 `mc-auth`，目标为 `https://github.com/AIH-MC/aihmc-service.git`。按用户要求，M0 至 M7 完成后统一提交推送。账号相关验收和实际生产切换、观察及回滚由接收方按手册人工完成，不阻塞本次代码交付。
