# Python 原版归档

原业务源码、`requirements.txt`、YAML 示例已原样移动到本目录，迁移时逐文件 SHA-256 核对一致。原项目说明见 `README.original.md`。本目录有独立 assets、keys、textures 和 SQL 文件，可从此目录单独部署。

配置仍为 YAML：复制 `config.example.yml` 为本目录的 `config.yml`，填写连接和 Token，配置现有公钥，当前目录设为本目录，再运行 `python main.py`。原有生产配置、私钥和缓存未纳入归档提交。

原 `requirements.txt` 完整保留。Windows Python 3.13 参考验证使用 `requirements-reference.txt`：补 Pillow，使用 bcrypt bytes API 提供方和 aiomysql 可调用的 PyMySQL 编码接口；原声明依赖在该环境的失败见根目录 `docs/PYTHON_BASELINE.md`，不声称原依赖清单在新 Python 上已修复。

安装命令按前台日志规范执行：

```powershell
& python -m pip install -r requirements-reference.txt 2>&1 | Tee-Object -FilePath "build.log"
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content build.log
    exit $buildExitCode
}
```

运行需要复用现有 MySQL/MariaDB。`database.sql` 是含 DROP DATABASE/TABLE 的历史导出，不可直接对已有业务库运行。回滚只切回本程序及 YAML 配置，不清库。
