# AIHMC API 后端 Go 迁移计划书

日期：2026 年 10 月 22 日

状态：M0 至 M5 自动验收通过；M6 按用户确认完成人工交接，生产操作非阻塞 MANUAL/PENDING；M7 实现与本地复验完成；最终推送和 Actions 结果由交付时核验

本计划将现有 PHP 后端迁移为 Go 服务，保留 Minecraft 外置登录、UCenter 用户体系、皮肤材质和头像能力。迁移以现有客户端可继续使用、用户身份不变、已有数据可复用、上线可回退为首要验收标准。Go 服务必须使用 TOML 配置文件。

用户已授权按本计划完成 M0 至 M7，并要求全部完成后统一提交推送。阶段证据记录于 `docs/MIGRATION_PROGRESS.md`。账号相关测试按用户指示作为非阻塞人工验收；用户另已确认“生产切换改为人工交接，不阻塞交付”。阶段完成代表相应代码与交接交付完成，不代表真实生产验收通过。

## 一 迁移目标与范围

### 迁移目标

1. 用一个 Go HTTP 服务承载现有正式业务接口，保持原有域名与路径。
2. 继续使用现有 MySQL 或 MariaDB 数据库及六张业务表，首期不重建用户体系、不替换数据库。
3. 保留已存储的用户 UID、profile_id、player_uuid、令牌及材质引用，不批量重新生成身份数据。
4. 继续使用现有 RSA 密钥对，对接现有 UCenter、皮肤站及第三方材质服务。
5. 用 TOML 管理运行参数，以独立进程部署，具备明确的启动失败提示、请求日志、停止和回滚步骤。
6. 先证明功能与兼容性，再比较性能；不预设语言迁移必然提高登录速度，外部服务耗时仍需单独测量。
7. 交付仓库固定为 `https://github.com/AIH-MC/aihmc-service.git`，工作与交付分支固定为 `mc-auth`。迁移完成后将旧 PHP 项目移入 `archive/`，活动源码及默认构建、测试和部署入口仅保留 Go；全部阶段完成后统一提交推送。
8. 完成交付时增加统一 GitHub Actions 工作流，仅编译 Linux x86_64，即 `GOOS=linux`、`GOARCH=amd64`，不增加其他平台构建矩阵。

### 首期范围

包含 `index.php` 暴露的接口、认证和会话状态、数据库访问、UCenter 签名及请求、材质下载与存储、PNG 头像处理、用户删除、配置与部署迁移，以及必要的兼容性验证。

不包含管理后台、新登录方式、多角色系统、JWT 改造、Redis、消息队列、微服务拆分、数据库换型、皮肤上传、独立 CSL 接口新增或历史数据清洗。仓库中存在的辅助函数不自动等于已发布接口。

只在新 Go 实现中完成必要的错误处理和参数化 SQL，不顺带重构现有 PHP。涉及对外行为、授权规则、数据库结构的变更，必须先确认；与迁移无关的问题单独记录。

## 二 当前系统基线

### 模块与依赖

| 当前位置 | 当前职责 | Go 迁移要求 |
| --- | --- | --- |
| `index.php` | 路由、请求解析、方法判断和响应输出 | 建立接口清单和实际响应基线 |
| `mc_auth/` | 登录、令牌、角色查询和进服验证 | 保留身份和状态流转 |
| `includes/classes/database.php` | mysqli 数据访问 | 显式字段映射和参数化 SQL |
| `includes/classes/format_data.php` | 登录与材质数据序列化 | 保留字段类型、可选字段和材质签名输入 |
| `includes/functions/uc.php` 与 UCenter 用户函数 | 上游签名、取 token、登录、查询用户 | 按现有 UCenter 协议迁移 |
| `skin_api/` | 皮肤源查询、头像生成和回退 | 保留查询顺序、合成效果与输出尺寸 |
| `system/deluser.php` | 由超级密钥授权的用户删除 | 保留授权边界和删除范围 |
| `keys/` 与 `textures/` | RSA 密钥和材质文件 | 作为外部持久化资源复用 |
| `config.php` | 站点、数据库、UCenter 和管理密钥配置 | 映射为 TOML 配置 |
| `nginx.htaccess` 与 `.user.ini` | 路由转发示例及 PHP 运行目录限制 | 实施时核对真实生产部署 |

当前代码依赖 PHP 的 mysqli、cURL、OpenSSL 和 GD 能力。数据库导出头部标注 MariaDB 10.6.16，不能据此认定当前生产数据库版本。仓库中未发现现成的自动化测试工程或发布流水线，实施前需要建立迁移验收基线。

### 数据表

| 表名 | 数据用途 | 迁移关注点 |
| --- | --- | --- |
| `mcauth_profiles` | UID、角色名、profile_id、player_uuid、材质数据 | 多个唯一约束；身份值与已有材质数据保持一致 |
| `mcauth_tokens` | access_token、client_token、创建时间 | UID、角色名、access_token 唯一；保留现有单用户记录模型 |
| `mcauth_logins` | server_id、令牌、角色名、IP、时间 | server_id 是主键；重复进服及清理行为需验证 |
| `mcauth_textures` | 玩家皮肤、披风及更新时间 | 空字符串、NULL、UUID 和材质引用需一致 |
| `mcauth_skin_urls` | 初始化接口公布的皮肤域名 | 与材质上游列表用途不同，不合并 |
| `texture_apis` | 外部皮肤 API 列表 | 保留实际查询顺序，核对数据库返回顺序是否稳定 |

`db.sql` 包含删除数据库和表的语句及业务数据，不能直接作为生产升级脚本执行。测试只使用隔离数据库和脱敏样本；真实数据、令牌、密码及私钥不得进入配置示例、测试仓库或日志。

## 三 接口迁移清单

以下路径来自当前入口，动态参数以花括号标识。状态码和异常响应以阶段 M0 的实测契约为准，不能仅由通用 Yggdrasil 文档推定。

| 方法 | 路径 | 迁移内容 |
| --- | --- | --- |
| 当前入口未限制 | `/` | 站点提示 JSON，核对非 GET 行为 |
| GET | `/mcauth` | 元信息、皮肤域名和签名公钥 |
| POST | `/mcauth/authserver/authenticate` | UCenter 登录、令牌绑定、角色与材质初始化 |
| POST | `/mcauth/authserver/refresh` | 令牌刷新和角色响应 |
| POST | `/mcauth/authserver/validate` | 令牌验证 |
| POST | `/mcauth/authserver/invalidate` | 令牌失效及重复请求 |
| POST | `/mcauth/authserver/signout` | 账号验证和退出 |
| POST | `/mcauth/sessionserver/session/minecraft/join` | 令牌与角色匹配、进服记录 |
| GET | `/mcauth/sessionserver/session/minecraft/hasJoined` | 角色名、serverId 和 IP 相关验证 |
| GET | `/mcauth/sessionserver/session/minecraft/profile/{uuid}` | 角色材质查询和 unsigned 参数 |
| POST | `/mcauth/api/profiles/minecraft` | 批量角色查询 |
| GET | `/mcauth/users/profiles/minecraft/{name}` | 单角色查询 |
| GET | `/mcauth/api/users/profiles/minecraft/{name}` | 单角色查询的第二条兼容路径 |
| GET | `/helm/{uuid}/{size}` | 本地头像合成或 Crafthead 回退 |
| POST | `/system/deluser` | 超级密钥验证与用户数据删除 |
| 静态资源 | `/textures/{hash}` | 现有材质下载 URL，由网关继续提供 |
| 当前默认分支 | 未匹配路径 | 当前返回初始化数据，需纳入兼容性决策 |

还需验证 URL 编码、中文角色名、查询字符串、尾斜杠、错误方法、空请求体、非法 JSON、缺失参数和无数据响应。反向代理当前是否允许直接访问 PHP 文件、静态资源的响应头和缓存规则，由 M0 核对真实部署后确定，不能只覆盖入口中的正常请求。

## 四 Go 技术方案

### 服务结构

采用单体服务，按 HTTP 处理、业务流程和外部资源访问划分职责，不照搬 PHP 的逐文件包含方式，也不引入通用框架层。

```text
现有客户端和 Minecraft 服务器
              |
       现有域名与反向代理
          /            \
   Go API 服务      /textures/ 静态资源
      |       \
   业务流程    UCenter 与材质上游
      |
   现有数据库与持久化材质目录
```

迁移期间先在 `go/` 目录建立 Go 模块，保留 PHP 原样以便对照与回退。完成迁移后，在归档阶段将 Go 模块提升至仓库根目录，将旧 PHP 项目移入 `archive/php/`，全部阶段完成后统一提交推送。最终布局如下：

```text
go.mod
go.sum
cmd/aihmc-api/          程序入口
internal/config/       TOML 加载和校验
internal/httpapi/      路由、请求与响应
internal/auth/         认证、令牌、角色和会话流程
internal/store/        数据访问与事务
internal/ucenter/      UCenter 协议适配
internal/texture/      皮肤源、下载、签名和头像
config.example.toml     唯一提交的配置模板
config.toml             实际运行配置，由已有配置迁移生成，不提交
tests/                 契约与集成验证
archive/php/           旧 PHP 源码与脱敏部署参考，仅供历史查阅
GO_MIGRATION_PLAN.md
```

按实际职责建立目录，不为占位提前创建抽象或空模块。“只保留 Go 代码”指活动工程仅使用 Go；`archive/php/` 是用户要求保留的历史归档，不参与构建、测试或部署，也不由 Web 网关公开。旧 PHP 保留在归档中；按用户确认，实际生产切换、观察及回滚转人工交接，不阻塞本次本地源码归档。已核验原文件摘要，生产回退仍须保留完整的受控 PHP 发布版本。

归档涵盖旧入口、`includes/`、`mc_auth/`、`skin_api/`、`system/` 和旧 PHP 部署参考。原始 `config.php`、私钥、含业务数据的 `db.sql`、运行材质和秘密文件不随归档推送；需要留作回退的原件存放于受控运行目录或备份中，仓库仅保留脱敏样例及必要的无业务数据 schema。归档前核验文件清单与备份，禁止直接移动仍被运行实例使用的目录。

### 技术选型建议

| 能力 | 建议 | 选择理由与约束 |
| --- | --- | --- |
| HTTP | 标准库 `net/http` | 当前接口规模有限，显式路由并处理默认行为、方法和编码差异 |
| 数据库 | `database/sql` 与 `go-sql-driver/mysql` | 复用 MySQL 协议和现有表；不引入 ORM 或自动改表 |
| 配置 | TOML 文件与专用 TOML 解析库 | TOML 为明确要求；解析库在 M1 核验后锁定版本 |
| 序列化 | `encoding/json` 加显式响应类型 | 控制数组、空值、缺省字段及数字类型 |
| 签名 | `crypto/rsa`、`crypto/sha1`、`crypto/x509` | 对现有签名做跨语言验证，不自行替换协议算法 |
| 图像 | `image`、`image/png`，按需要选择缩放实现 | 以现有 GD 结果对照，不能默认普通缩放等价 |
| 日志 | 标准库结构化日志能力 | 记录路径、状态、耗时及错误来源，脱敏敏感字段 |
| 部署 | Go 可执行文件与现有网关 | 复用现有运维方式，不强制引入容器或集群 |

Go 工具链及第三方依赖在 M1 开始时选择仍受支持且与目标操作系统兼容的版本，并记录、锁定；本计划不写死未经目标环境验证的版本号。

官方资料：[Go 数据库访问](https://go.dev/doc/database/)、[MySQL 驱动](https://github.com/go-sql-driver/mysql)、[HTTP 标准库](https://pkg.go.dev/net/http)、[RSA 标准库](https://pkg.go.dev/crypto/rsa)。这些资料支持组件能力，具体兼容性仍由本项目的验收用例证明。

## 五 TOML 配置设计

### 加载规则

服务支持 `--config <路径>`，未指定时读取工作目录下的 `config.toml`。相对文件路径统一相对于配置文件所在目录解析，避免服务管理器改变工作目录后找不到密钥或材质。

配置文件仅提交仓库根目录的 `config.example.toml` 模板，内容使用示例值和占位符。实际运行读取 `config.toml`，不得读取模板作为运行配置，也不得在缺少实际配置时自动回退到模板。后续实施时将现有 `config.php` 中的真实配置逐项迁移到 `config.toml`，保留现有业务参数和凭据；采用秘密文件引用的字段，将原值迁入相应秘密文件并在 `config.toml` 中填写路径，不以模板默认值覆盖已有值。

已将 `config.toml` 及其备份、实际秘密文件加入 `.gitignore`，提交前仍需核对暂存清单，确保只有 `config.example.toml` 配置模板进入版本控制。实际配置仅保存在受控运行环境和备份中，不随 `archive/` 归档提交或推送。M1 已完成本地配置迁移及逐项核对，证据见实施记录。

启动时一次性加载 TOML 并校验：语法、未知字段、必要字段、监听地址、超时、数据库连接、密钥格式和公私钥匹配、材质目录访问权限。失败时输出字段名和原因并以非零状态退出，不打印秘密值。不实现运行时热更新，变更配置后通过受控重启生效。

数据库密码、UCenter secret 和超级密钥通过 TOML 中的文件路径引用外部秘密文件；示例只保留占位路径。秘密文件内容按明确规则读取，允许移除末尾换行但不能任意裁剪密码中的其他空白。实际配置、秘密文件和私钥不提交 Git。普通配置不增加另一套环境变量覆盖机制，减少优先级歧义。

### 配置示例

以下为根目录 `config.example.toml` 的配置样例。监听地址、超时和连接池数值是初始建议；实际运行配置已从旧配置迁移，部署前按目标环境核对，不能用模板值覆盖现网值。

```toml
[server]
listen = "127.0.0.1:8080"
public_base_url = "https://api.example.com"
read_header_timeout = "5s"
read_timeout = "15s"
write_timeout = "30s"
shutdown_timeout = "15s"

[site]
name = "AIHMC API"
implementation_name = "aihmc-api-go"
implementation_version = "0.1.0"
homepage = "https://www.example.com"
register_url = "https://www.example.com/register"
allow_non_email_login = true

[database]
host = "127.0.0.1"
port = 3306
name = "aihmc_api"
user = "aihmc_api"
password_file = "secrets/database-password"
charset = "utf8mb4"
collation = "utf8mb4_unicode_ci"
max_open_connections = 10
max_idle_connections = 5
connect_timeout = "5s"
query_timeout = "5s"

[ucenter]
endpoint = "https://ucenter.example.com/api"
app_id = "replace-me"
secret_file = "secrets/ucenter-secret"
nonce = "replace-with-existing-value"
request_timeout = "10s"

[auth]
private_key_file = "keys/yggdrasil-private-key.pem"
public_key_file = "keys/yggdrasil-public-key.pem"
login_record_retention = "30m"

[textures]
directory = "textures"
public_base_url = "https://api.example.com/textures"
request_timeout = "10s"
mojang_base_url = "https://api.mojang.com"
crafatar_base_url = "https://crafatar.com"
crafthead_base_url = "https://crafthead.net"

[admin]
super_key_file = "secrets/admin-super-key"

[logging]
level = "info"
format = "json"
```

`login_record_retention` 的示例值不代表现网值；迁移时从 `$delete_old_record_before` 的秒数转换得到等价时长，不顺带改变清理触发时机。外部皮肤 API 列表与初始化接口的皮肤域名仍从各自数据库表读取，不复制到 TOML 制造双重来源。

### PHP 配置映射

| PHP 配置 | TOML 对应项 |
| --- | --- |
| `$name`、`$impname`、`$impver` | `site.name`、`site.implementation_name`、`site.implementation_version` |
| `$homepage`、`$reg_url`、`$allow_non_email` | `site.homepage`、`site.register_url`、`site.allow_non_email_login` |
| `$delete_old_record_before` | `auth.login_record_retention`，单位转换但语义不变 |
| `$host`、`$port`、`$user`、`$pass`、`$dbname` | `database` 对应项，密码改由 `password_file` 引用 |
| `$api_endpoint`、`$appid`、`$secret`、`$nonce` | `ucenter` 对应项，secret 改由 `secret_file` 引用 |
| `$super_key` | `admin.super_key_file` 引用的文件内容 |
| 代码内材质域名、密钥路径、材质目录 | `textures.public_base_url`、`auth` 密钥路径、`textures.directory` |

实施时逐项核对现网值和类型，尤其是布尔值、秒与毫秒、UID 数值及字符串 app_id。签名算法、鉴权规则不设计成可任意切换的配置项。

## 六 兼容性重点与待决策行为

### 身份和令牌

已有 UID、profile_id 和 player_uuid 以数据库记录为准。当前 UUID 生成函数是随机 v4，不能替换成由用户名推导的离线 UUID。新用户采用相同格式；老用户再次登录时，返回身份、材质 payload 中的 profileId 和数据库映射必须一致。当前重复登录的资料写入逻辑需在 M0 用老用户用例验证，发现不一致时先确认最小修正方案。

现有 accessToken 不是 JWT，首次生成和刷新使用不同的哈希与 Base64 组合。首期保留合法旧令牌的识别与当前状态模型，不主动增加过期机制或强制用户全部重新登录。用跨 PHP 与 Go 的令牌互认用例验证回滚能力。新令牌生成方案若需改变，作为显式决策，不在迁移中暗改。

### 签名与序列化

现有材质签名是对 Base64 材质字符串执行 SHA1withRSA，再对签名进行 Base64 编码。使用原密钥建立固定测试向量，验证 PHP 签名由 Go 校验、Go 签名由 PHP 校验；保留已有 `texture_data` 的原始字符串，避免解析再序列化改变签名输入。

UCenter 签名为 `Base64(SHA256 十六进制文本(nonce + timestamp + secret))`，不能误写成对 SHA256 原始摘要字节做 Base64。必须检查头名称、时间单位、请求体和上游业务状态。

普通 JSON 按字段及类型比较，可忽略无意义空白和键顺序；签名输入逐字节比较。明确 `[]`、`null`、字段缺失和 HTTP 204 空响应体的区别。

### 需在 M0 固定处理结论

| 观察到的实现 | 决策要求 |
| --- | --- |
| 入口多处直接匹配完整 REQUEST_URI，部分路径使用前缀匹配；未知路径返回初始化内容 | 测试查询串和尾斜杠，不默认为 REST 风格 404 或自动重定向 |
| profile 查询使用 PHP 真值判断 `unsigned` | 覆盖缺省、空值、`0`、`true`、`false` 字符串，不直接用 Go 布尔解析替代 |
| 入口向 `validate_client` 传 IP，但函数仅声明两个参数且内部使用未定义的 `$ip` | 验证当前运行行为；是否正式启用 IP 限制需确认，不能借迁移强化规则 |
| 删除接口读取 `prarms.uid` | 首期保留此拼写；是否另支持 `params` 属于接口扩展 |
| token 绑定更新分支仅更新 client_token 和创建时间；刷新更新条件使用 client_token | 验证重复登录、共享 clientToken 和并发刷新；变更更新范围需形成决策 |
| 登录后写入令牌、材质和资料，删除用户跨多表执行 | Go 中使用必要事务保证本地写入一致性；事务外完成上游请求，验证失败回滚行为 |
| 上游请求关闭 TLS 验证并使用较长超时 | Go 拟使用正常证书验证和有限超时；先验证现网上游证书与可用性，存在依赖冲突时暂停并确认，不偷偷关闭验证 |
| 头像采用 GD 重采样与叠层，皮肤模型判断有固定像素规则 | 通过固定 PNG 样本确认效果，不在迁移时改造图像算法 |

这些是兼容性调查项，不是已经证明的生产故障或安全结论。行为结论记录为“保留”“经确认修正”或“不属于受支持行为”，附用例和理由；存在未决项时不得靠随意选择一种实现推进相关模块。

## 七 数据与上线迁移方式

首期采用同数据库结构、同资源目录、同签名密钥的替换式部署。测试环境使用独立副本；Go 服务先在独立端口运行。生产通过现有网关切换整个认证业务，避免按单条路由拆分认证状态链路。

1. 备份生产数据库、材质、密钥和网关配置，验证备份可恢复；不要执行仓库中的破坏性 SQL 导入。
2. 按 TOML 映射将已有 `config.php` 配置迁移为实际运行的 `config.toml`，核对值、类型和秘密文件路径；只提交 `config.example.toml` 模板，实际配置及秘密文件保持忽略。通过受限文件权限加载秘密，复核数据库字符集和真实 schema 与导出的差异。
3. 在隔离环境完成数据恢复、旧令牌互认、真实 UCenter 和客户端联调，并演练 Go 写入后切回 PHP。
4. 生产 Go 先完成启动和依赖检查，不复制执行登录、刷新、进服或删除等真实写请求。只读对照也应核实没有间接写副作用。
5. 切换前短暂收口新请求并等待 PHP 在途请求结束，记录切换时刻；再将认证接口整体转向 Go。材质静态 URL 保持可达。
6. 按约定窗口观察登录成功率、进服成功率、上游错误、响应耗时和数据库错误，异常时执行回滚。

回滚时先收口 Go 新请求并等待在途写操作结束，再恢复 PHP 网关配置和进程，验证登录、刷新、进服和材质。常规回滚不恢复旧数据库快照，以免抹掉切换后的有效写入。只有确认存在数据损坏且经过决策者批准，才执行独立的数据恢复方案。

M0 必须约定允许的中断时长、观察窗口和回滚触发阈值；M5 必须提供演练结果。共享 schema 并不自动证明双向兼容，未验证 Go 生成数据可由 PHP 正确读取前，不得宣称可无损回滚。

## 八 实施里程碑与验收

所有阶段严格按顺序执行，实施已获授权；每阶段的验证和进度记录仍需独立完成，提交与推送按最新指令延后至全部阶段完成。

| 阶段 | 工作和交付物 | 必须通过的验收 | 初估投入 |
| --- | --- | --- | --- |
| M0 基线与决策 | 接口契约、脱敏用例、现网依赖清单、兼容决策、上线阈值 | 关键状态链路基线可复现；身份和签名向量建立；未决行为有明确结论 | 2 至 3 人日 |
| M1 服务与 TOML 配置 | Go 模块、`config.example.toml` 模板、已有配置迁移为不入库的 `config.toml`、忽略规则、启动校验、HTTP 基础和数据库连接 | 运行读取 `config.toml` 且迁移值一致；缺失时不回退模板；非法与未知字段失败；相对路径正确；正常启停；仅模板入库且秘密不泄漏 | 2 至 3 人日 |
| M2 数据与协议适配 | 六表访问、UCenter 客户端、序列化与签名 | 真实数据库集成通过；签名互验、UCenter 签名向量和上游失败映射通过；必要事务回滚正确 | 3 至 4 人日 |
| M3 认证与会话接口 | 初始化、登录、刷新、校验、失效、退出、角色查询、join 和 hasJoined | 正反向契约与状态序列通过；旧数据、中文身份、重复登录、并发刷新和令牌互认通过 | 4 至 6 人日 |
| M4 材质头像与删除 | 多源材质回退、文件存储、头像与管理删除接口 | 皮肤与披风缺失及上游失败覆盖；图像对照通过；删除鉴权、数据范围和失败回滚通过 | 3 至 4 人日 |
| M5 联调与发布准备 | 全量接口对照、负载基线、部署与回滚手册，账号项目转人工 | 全量自动检查通过；双向令牌读取验证通过；本地性能样本记录；账号相关真实链路及回滚为非阻塞人工验收 | 3 至 5 人日 |
| M6 生产交接 | 按用户确认交付部署、切换、观察及回滚手册和执行记录模板；实际生产操作转人工 | 交接材料齐备；实际切换、观察和账号验收明确记为非阻塞 MANUAL/PENDING，不声明已上线 | 人工执行时间另定 |
| M7 旧代码归档与最终交付 | 将旧项目归档到 `archive/php/`，Go 模块提升到根目录，更新路径和文档，增加统一 Linux x86_64 GitHub Actions，统一提交推送 `mc-auth` | M6 验收通过；根目录构建与测试通过；部署不再依赖旧目录；归档无敏感数据；工作流只编译 Linux amd64，产物不含运行配置或秘密；最终远端提交与本地一致 | 0.5 至 1 人日 |

总计约 18.5 至 28 人日，按一名熟悉 Go 和当前业务的开发者估算，不包含环境或账号等待、未决产品行为讨论和新增功能。M0 后按实际依赖和基线修订，不把估算当作交付承诺。项目负责人确认行为取舍与上线窗口，开发者负责实现和证据，运维负责真实部署及回滚，验收人员负责客户端验证。

### 阶段检查点

每阶段完成所需自动化验证后，更新本计划中的进度和证据，检查 `git status`、`git diff` 与 `git diff --check`，通过该阶段验收后再进入下一阶段。按用户最新指令，中途不提交或推送；M0 至 M7 全部完成后，在 `mc-auth` 分支统一提交并推送到 `https://github.com/AIH-MC/aihmc-service.git` 的同名分支，核对远端提交。不向 `main` 推送，不自动合并其他分支。

初始 main 分支无提交且未配置远端。实施时已确认指定远端可访问且无引用，建立尚无提交的 `mc-auth` 分支并添加 origin。最终提交前再次核对远端，保留期间可能出现的已有内容；禁止强推或覆盖历史。尚未提交或推送，不得以一次性 `git add .` 把配置、私钥和 SQL 业务数据纳入版本控制。

完成 M6 人工交接材料后执行 M7 归档、根目录调整和复验，最后统一提交并推送 `mc-auth`，确保 Go 实现和归档结果同时进入目标仓库。

按用户最新指示，凡需要账号的真实 UCenter 登录、启动器、客户端进服、皮肤披风加载及账号相关回滚验证，均记录为 `MANUAL/PENDING`，属于人工验收，不阻塞本次代码交付与阶段检查点。自动检查仍须通过；人工项目未执行时不得声称真实链路或上线验收通过。按用户随后确认，生产切换、观察及生产回滚改为非阻塞人工交接，具体步骤和执行记录见 `docs/DEPLOYMENT.md`；将来操作时仍须具备生产访问资料和批准窗口。

## 九 验证与完成标准

### 验证层次

| 层次 | 验证内容 | 证据边界 |
| --- | --- | --- |
| 函数与固定向量 | TOML、签名、序列化、UUID 格式、图像样本 | 证明局部逻辑，不代表真实登录或进服成功 |
| 数据库与 HTTP 集成 | 独立 MySQL 或 MariaDB、事务、路由、状态码、响应头、文件访问 | 证明本地业务链路和真实数据访问 |
| PHP 与 Go 契约对照 | 同一初始数据、同一受控上游响应，分别运行完整请求序列 | 比较返回值及最终数据库、材质状态，不能对同一库重复执行写请求 |
| 真实联调 | 测试 UCenter、已支持启动器和 Minecraft 服务端、皮肤加载 | 证明实际受支持场景可用；使用测试账号和隔离资源 |
| 部署演练 | 启动、停止、切换、配置路径、权限、回滚 | 证明目标环境可以发布和恢复 |

认证至少覆盖：首次与重复登录、错误密码、大小写和中文、缺失 clientToken、requestUser 分支、无效令牌、刷新后旧令牌、失效及退出后验证、错误 selectedProfile、hasJoined 成功与失败、过期登录记录，以及重启后已有令牌继续可读。预期结果遵循 M0 确认的规则。

图像至少覆盖：64×32 与 64×64 皮肤、不同模型、透明帽子层、披风缺失、材质站不可用、Mojang 无此用户、损坏 PNG、本地命中及外部头像回退。比较 PNG 解码后的尺寸、像素和叠层效果；压缩字节不同不自动判为失败，容差如有需要须先固定。

管理删除仅在隔离数据上验证，覆盖空密钥、错误密钥、不存在用户和成功删除，检查影响范围。不对生产账号执行验收删除。

### 构建与测试纪律

后续全部构建、编译、打包和测试都在前台 PowerShell 执行，实时输出并保存 `build.log`，立即捕获退出码，失败时读取日志定位。每条命令分别遵循以下模式：

```powershell
& go test ./... 2>&1 | Tee-Object -FilePath "build.log"
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath "build.log"
    exit $buildExitCode
}
```

对 `go build`、`go vet` 和适用环境下的并发检查分别执行同样流程，不使用后台构建或隐藏输出。日志含测试身份信息时按敏感资料管理，保留各阶段必要记录。

后端 HTTP 和 PNG 样本验证不依赖 Qt。如联调使用 Qt 启动器，必须启动真实 Windows 窗口，等待稳定渲染，交互并按需截图检查中文和皮肤显示，记录结果后关闭；禁止 Qt offscreen。无法完成真实窗口验证时明确标记 `MANUAL/PENDING`，不能替代为通过。

### 最终完成条件

全部已确认接口和数据副作用通过本地契约验证，老用户身份与既有令牌兼容；TOML 配置迁移、加载及数据库连接通过；部署、切换、观察及回滚交接材料齐备；旧 PHP 已归档且活动工程仅使用 Go；Linux x86_64 Actions 构建通过；最终提交已推送到指定仓库的 `mc-auth` 分支并核验。按用户确认，需要账号的真实客户端测试和实际生产操作均为非阻塞 `MANUAL/PENDING`，交付完成不等于这些人工项目已通过。不追加范围外的优化作为交付门槛。

## 十 风险与开工前确认

| 风险 | 控制方式 |
| --- | --- |
| PHP 弱类型、URL 匹配与 Go 默认行为不同 | 先固定契约，对需要修正的行为逐项确认 |
| 重建身份或重编码材质导致角色、皮肤或签名变化 | 复用身份与已存材质字符串，增加跨语言固定向量 |
| UCenter、材质站不可用或证书不符合预期 | M0 核实可用环境；明确超时及失败处理，不能用模拟结果代替真实联调 |
| Go 并发使原有更新顺序和唯一约束冲突暴露 | 用真实数据库覆盖竞争写入；只做满足当前状态模型的必要事务处理 |
| Go 写入的数据使 PHP 回滚后无法使用 | M5 必须演练双向读取和完整状态序列 |
| 接口以外的生产网关行为遗漏 | 记录真实配置、直接 PHP 路径与静态资源规则 |
| Git 检查点无法推送 | 开工前验证指定远端和 `mc-auth` 分支访问权限，建立源码基线，不跳过阶段检查点 |

开工前需确认：生产操作系统和进程管理方式、真实网关及数据库配置、UCenter 测试环境与测试账号、当前受支持的启动器和服务端版本、兼容性差异结论、可接受切换窗口及验收阈值、源码基线和指定远端的访问权限。可以先完成不依赖这些信息的 M0 文档调查，不能以未确认的假设执行生产操作。

## 十一 当前进度

- [x] 完成仓库静态梳理和 Go 迁移计划草案。
- [x] 纳入 TOML 配置文件、分组样例、PHP 配置映射和加载校验要求。
- [x] 明确只提交 `config.example.toml` 模板，实际运行读取由已有配置迁移生成且不入库的 `config.toml`。
- [x] 明确目标仓库、`mc-auth` 分支、旧代码归档和最终 Go 工程布局。
- [x] 纳入统一 GitHub Actions，仅编译 Linux x86_64 的交付要求。
- [x] 计划评审与实施授权，按最新指令全部完成后统一提交推送。
- [x] M0 本地基线与兼容决策；现网资料及上线阈值 MANUAL/PENDING，详见实施记录。
- [x] M1 服务与 TOML 配置，已迁移实际配置并通过加载、连接和实际进程启停验证。
- [x] M2 数据与协议适配，真实数据库事务、协议向量及 RSA 双向跨语言验签通过。
- [x] M3 认证与会话接口，HTTP 状态链路、旧令牌双向互认、持久化及并发验证通过。
- [x] M4 材质头像与删除，回退、GD 像素对照、并发发布和删除接口验证通过。
- [x] M5 联调与发布准备，39 项实际 HTTP 对照及本地全量检查通过；账号相关项目为非阻塞 `MANUAL/PENDING`。
- [x] M6 生产交接，按用户确认实际切换、观察及生产回滚转非阻塞人工执行。
- [x] M7 旧代码归档、根目录 Go 工程、Linux x86_64 Actions 及本地复验完成；最终提交、推送和远端 Actions 结果在交付时核验。

实施验证包含旧 PHP 与 Go 的实际 HTTP 对照、现有数据库中的隔离测试、协议和图像参考、程序启停、Linux 交叉编译及真实 UCenter 的有限连通性检查。具体证据及范围见实施记录；真实用户成功登录、真实客户端进服和生产切换仍为非阻塞人工验收，不宣称生产已切换或真实链路已通过。
