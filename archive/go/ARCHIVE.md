# Go 版本独立归档

生产 Go 源码基线为 cc458ab7d709323bdf4b9ca6f7ac1c857c4cfa09；截至 Rust 迁移 R4 提交 c01473979facddf2d09ce9816a818c79fa5663b8，Go 生产源码未改动。原 README、部署文档和 Go 工作流从基线恢复，保留原有字节和行尾；完整列表见 ARCHIVE_MANIFEST.json（58 个原文件及 2 个本次迁移校验助手）。

本目录包含独立 go.mod/go.sum、cmd、internal、tests、docs、配置模板和原 Go 迁移计划。嵌套 .github/workflows/build.yml 是历史快照，不会被当前 GitHub Actions 注册；其旧 push 触发方式不适用于当前项目。当前活动入口仅构建根目录 Rust，两个根工作流均只允许人工触发。

## Windows 构建与复验

进入 archive/go，在已有 Go 工具链环境前台执行：

```powershell
$env:GOOS = 'windows'
$env:GOARCH = 'amd64'
& go build -trimpath -o ../../.local/rust-migration/aihmc-api-go-archive.exe ./cmd/aihmc-api 2>&1 | Tee-Object -FilePath 'build.log'
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath 'build.log'
    exit $buildExitCode
}
```

完整测试使用 AIHMC_TEST_CONFIG 指向外部合成配置的绝对路径，数据库仅使用已有实例中的 aihmc_migration_test；执行 go test -count=1 -p 1 -v ./... 并遵循同样的 Tee/退出码模式。配置和密钥不在归档内，必须使用受控运行资源，不能运行配置模板兜底。Go-to-PHP 反向执行是单独的可选测试，需显式提供 PHP 和隔离旧项目路径。

测试助手：tests/tools/protocol-bridge 用于双向 RSA 验签，tests/tools/http-fixture 用于受控材质的 handler 对照；从本目录 go build 相应包即可，仅用于合成数据，不包含在服务发布包中。

## 文件与回退

ARCHIVE_MANIFEST.json 的 archive 路径相对于当前仓库根目录。对每个文件执行 SHA256 必须与清单一致；.gitattributes 禁止对归档文本自动转换行尾。不要对历史源文件执行批量格式化。

不包含实际 config.toml、秘密、私钥、业务 SQL、运行材质或缓存。PHP 源码在同级 ../php；历史 Go 文档中的 archive/php 路径描述的是旧仓库布局。运行时回退优先使用受控保留的 Go 发布包，参见根目录 docs/DEPLOYMENT.md；当前源码归档不代表已执行生产回退。
