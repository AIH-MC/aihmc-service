# PHP 历史实现

此目录保存迁移前的 PHP 源码与网关配置，仅用于历史参考和人工回退核对，不参与 Go 服务的构建或运行。

原始 `config.php`、`db.sql`、RSA 密钥、秘密及材质数据不进入 Git。原业务 SQL 含删除语句和业务数据，不能当作 Go 升级脚本。六表的无数据参考结构见仓库根目录 `tests/schema.sql`。

38 个原始 PHP 源文件的 SHA256 记录于 `docs/legacy-source-sha256.json`，路径相对于本目录；归档时已核对内容未变。

实际生产回滚需要保留的完整 PHP 发布版本、原运行配置及数据，由部署人员按 `docs/DEPLOYMENT.md` 操作。本归档不是可直接部署的完整生产备份。
