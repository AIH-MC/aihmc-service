# Python 行为基线（阶段 1）

运行于 Windows x64，Python 3.13.12。生产 Python 源码保持不变。

## 运行

创建 `.venv` 并安装 `requirements-test.txt`。本地 `config.yml` 提供现有 MySQL/MariaDB 连接，文件已忽略。测试只创建并删除 `aihmc_migration_test_<随机32位hex>` 库，不执行原 SQL 文件的 DROP 指令，也不读取业务数据。

```powershell
$env:PYTHONUTF8 = '1'
& .\.venv\Scripts\python.exe -m unittest tests.test_python_baseline tests.test_mysql_baseline -v 2>&1 |
    Tee-Object -FilePath "build.log"
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content build.log
    exit $buildExitCode
}
```

测试配置注入仅用于 Python 参考测试，不读取生产 Token/密钥。路由测试用 AsyncMock 分隔协议层，数据库测试另接真实 MariaDB；回环测试实际启动 Uvicorn 子进程，并向 HTTP 端口发送请求。受控上游使用真实本地 HTTP 服务，测试结束清理端口和进程。

## 证据范围

- 20 个业务路由：成功/失败状态及消息适配、鉴权、缺字段、CORS、IP、PNG 文件响应；业务适配样本不作为完整业务端到端证据。
- 算法：离线 UUID、大小写、时长、标识符、Unicode/长密码、透明 PNG 叠加与最近邻缩放。
- 来源：四种 hasJoined URL、禁用来源和公共后缀；实际回环请求保留当前 IP 拼接结果。
- 数据库：注册/重复注册、登录/改密、资料关联副作用、皮肤存取、Figura 同步及未注册回退、INSERT IGNORE 冲突。
- 文档端点：现有 `/openapi.json`、`/docs`、`/redoc` 均返回 200。Rust 需要保留可用的等价文档入口。

## 依赖和已证实差异

原 requirements 的 `python_bcrypt==0.3.2` 在当前 Python 上对 bytes 调用抛出 TypeError；PyMySQL 1.2.4 的 `escape_bytes_prefixed` 是字符串，而 aiomysql 0.3.2 将其作为函数调用，导致 bytes 密码写库失败。参考测试环境单独使用 bcrypt 4.3.0、PyMySQL 1.1.2，并补充源码已用的 Pillow 13.0.0；此为 Python 对照环境，不是最新版兼容完成，也未更改生产依赖。

已通过最小样本确认，Rust 先维持以下协议/数据行为，缺陷修复另行决定：

1. 资料关联写库成功后函数返回 None，HTTP 层仍返回 400。
2. 空皮肤记录查询抛出 IndexError。
3. 带 IP 的 hasJoined 实际查询为 `serverId=s?ip=192.0.2.1`，不是独立 ip 参数；Figura 上游不发送 IP。
4. INSERT IGNORE 遇到同名记录仍返回执行成功，第二个 UUID 不会因此创建 namelink。
5. 长密码参考 bcrypt 按 72 字节截断；Rust 必须跨语言验证，不能默认换成拒绝长密码。
6. PNG 叠加采用 Pillow paste(mask=overlay)，半透明样本结果 alpha=191；不能直接换成通常的 alpha compositing。

密码处理和数据库错误在 Rust 中必须保持受支持业务可用，不复制 Python 依赖包的 API 失配。无界重试不作为运行正确性的要求；后续若需改变业务失败响应，先记录并确认。

## 配置变更决定

用户已明确要求 Rust 改用 TOML：交付 `config.example.toml`，部署使用忽略的 `config.toml`，不提供自动 YAML fallback。Python 基线保留 YAML 作为历史参照。配置值、来源顺序和默认值保持兼容，格式迁移单独说明；私有配置转换不输出凭据。

## 验证状态

阶段完成时以 `build.log` 的前台 unittest 输出记录结果。Linux 与 Actions 未运行，真实插件/上游账号为 MANUAL/PENDING，不宣称已验证。
