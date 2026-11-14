# Rust 服务部署与回退

本手册适用于从归档 Go 服务切换到当前 Rust 服务。配置、六张业务表、身份、令牌、RSA 密钥、材质持久化目录及既有 URL 继续复用，不需要业务 schema 升级。实际生产切换尚未执行，属于非阻塞 MANUAL/PENDING。

## 发布文件与验证状态

| 平台 | 产物 | 状态 |
| --- | --- | --- |
| Windows x64 / MSVC | aihmc-service-windows-x64.zip + .sha256 | 本地实际构建和业务验证，详见 Rust 实施记录 |
| Linux x64 / musl | aihmc-service-linux-x64.tar.gz + .sha256 | 构建配置已交付，人工 Actions/目标机验收 MANUAL/PENDING |
| Linux ARM64 / musl | aihmc-service-linux-arm64.tar.gz + .sha256 | 构建配置已交付，人工 Actions/目标机验收 MANUAL/PENDING |

发布包仅允许二进制、配置模板、README 和本手册。运行配置、秘密、私钥、业务数据、材质目录和 archive 不入包。核对架构及 SHA256 后部署到独立发布目录，保留原 Go 可执行文件和配置，以便回退。不要把源码归档当作已部署的 Go 发布版本。

Windows 安装 x64 VC++ 运行库，当前二进制依赖 VCRUNTIME140.dll；由现有进程管理方式运行，不新增 Windows 服务管理器。Linux 构建配置指定 musl 静态链接，脚本将验证无动态解释器/NEEDED；人工确认实际运行结果，并保留系统受信任 CA 证书以验证上游 HTTPS。Agent 不执行 Linux 构建或 Actions。

## 配置和资源

逐项复用已有 TOML 值，不用模板覆盖现网配置。默认读取 config.toml，--config 可显式指定；所有相对秘密、密钥和材质路径以配置文件目录为准。--check-config 检查配置、密钥对、材质目录和数据库；--help 不连接数据库。缺少实际配置时启动失败。

数据库仅连接已有 MySQL/MariaDB；普通启动不建库、不建表、不自动迁移。生产账号需要现有六表的实际业务读写权限，不执行 tests/schema.sql、旧 db.sql 或测试触发器。运行账号能读取秘密及私钥，材质目录可写，网关能读取公开材质。配置变更后受控重启。

发布前备份数据库、材质、密钥、实际配置和网关设置并确认恢复方法。备份保存在受控运行环境，不提交源码。

## 网关

保持现有域名、TLS 和 /textures/ 静态路由。示例只用于核对，不会自动应用到生产：

```nginx
location ^~ /textures/ {
    alias /absolute/persistent/textures/;
    default_type image/png;
}

location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
}
```

proxy_pass 不替换 URI，保留原始路径和查询串。网关覆盖 X-Real-IP；服务只信任回环 TCP 对端提供的该头，远程直连不能通过伪造此头改变记录 IP。hasJoined 保留既有不校验 IP 的行为。其他代理拓扑按实际环境单独核对。

仓库、archive、配置、私钥和秘密不作为静态 Web 根目录。材质静态响应头和缓存规则沿用现网配置，实际结果待人工核验。

## 切换

1. 人工取得正确架构的正式包并核对提交、SHA256、权限和必要运行环境。Linux 包由人工运行 Build Rust (manual) 获得；Agent 不触发。
2. 将 Rust 放在独立发布目录，复用实际配置、数据库、原密钥和持久化材质；在目标机执行 --check-config。
3. 在独立端口启动，验证元信息、公钥和已存在数据；用测试账号检查登录、刷新、进服、角色、头像及皮肤披风。客户端如使用 Qt，必须真实 Windows 窗口，禁止 offscreen。
4. 按人工确认的切换窗口收口 Go 新请求，等待在途写入结束，再整体切换认证路由到 Rust。不要复制执行生产写请求做影子对照。
5. 保持 /textures/ 可用，检查日志及客户端 IP。Windows 使用 Ctrl+C/Ctrl+Break，Linux 使用 SIGINT/SIGTERM 正常退出，应用按 shutdown_timeout 等待在途连接。
6. 按实际约定的观察窗口、成功率、延迟和回退条件观察。这里不预设未经批准的生产阈值。

目标验证从 PowerShell 前台执行并保存日志，例如：

```powershell
& ./aihmc-api.exe --config 'C:/service/config.toml' --check-config 2>&1 | Tee-Object -FilePath 'build.log'
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath 'build.log'
    exit $buildExitCode
}
```

需要在其他系统编译/测试时也须前台 PowerShell、Tee-Object 和立即检查退出码。普通长期运行进程由现有人工确认的管理方式托管，不把编译测试放到后台。

## 回退到 Go

收口 Rust 新请求并等待在途写操作结束，再将网关指回已保留的 Go 发布版本。继续使用同一配置、业务库、密钥和材质，验证既有及切换后生成的令牌、登录/刷新、进服和资源。

本地已通过 Go/Rust 双向令牌读取、刷新和持久化身份对照，覆盖受控测试场景；真实网关与客户端回退仍为 MANUAL/PENDING。常规代码回退不恢复切换前数据库快照，以免抹掉切换后的有效写入。数据恢复必须作为独立操作另行决策。

Go 源码、原模块锁、测试/样本、模板和原始工作流均在 ../archive/go/，恢复构建方法及文件清单见其 ARCHIVE.md 与 ARCHIVE_MANIFEST.json。归档不是当前 Cargo 或 Actions 的构建依赖；旧 PHP 保留于 ../archive/php/。

## 人工交接记录

以下均为非阻塞 MANUAL/PENDING，不代表已上线或已完成人工验收：

| 项目 | 人工填写 |
| --- | --- |
| Actions | 选择 mc-auth / Build Rust (manual)，记录 run、提交、三平台结果及 SHA256 |
| 目标环境 | 主机标识、OS/架构、进程管理方式、网关和持久化目录 |
| 配置预检 | 实际配置位置、运行账号、check-config 结果和日志 |
| 真实账号/客户端 | UCenter 登录、刷新/退出、进服、头像和皮肤披风 |
| 生产切换 | 批准窗口、备份点、收口和路由切换时间、执行人 |
| 观察 | 事先约定的指标/窗口/回退阈值，实际结果 |
| 回退 | 保留的 Go 发布包、实际恢复命令、账号及资源验证 |
| 上线决定 | 验收人、结论和遗留项；不要填写密码、私钥或用户令牌 |
