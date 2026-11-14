# Rust 迁移计划书

日期：2026-11-14
仓库：https://github.com/AIH-MC/aihmc-service
分支：`aihmc-auth`
状态：迁移实施中；阶段 1—3 已完成 Windows 自动验证。

## 1. 目标与范围

将当前 Python/FastAPI 后端迁移为 Rust 服务，兼容现有插件接口、配置、数据库和资源目录。

| 平台 | Rust target | 验证位置 |
| --- | --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` | 本地编译、测试；手动 Actions 构建 |
| Linux x64 | `x86_64-unknown-linux-gnu` | 手动 Actions 编译、测试 |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | 手动 Actions 编译、测试 |

Linux ARM 按 ARM64/aarch64 规划。本地仅验证 Windows x64，不要求 WSL、Docker 或 Linux 交叉工具链。

用户已授权按本计划实施 Rust 迁移，本地仅编译测试 Windows x64，不运行 Actions。后续工作流必须仅使用 `workflow_dispatch` 手动触发；禁止配置 push、pull_request、tag、schedule 或其他自动触发入口。实施和提交推送也不得擅自触发 Actions。

不新增业务功能、不改造插件、不重设计数据库、不引入管理页面或自动生产部署。ARMv7、musl 静态发行包、Windows ARM 不在本轮范围。

## 2. 现状与迁移对应

| 当前模块 | 现有职责 | Rust 对应 |
| --- | --- | --- |
| `main.py` | 路由、Token、CORS、IP、Uvicorn 启动 | HTTP 路由、鉴权与入口 |
| `libs/config_loader.py` | YAML 配置 | 配置结构与加载 |
| `libs/database.py` | aiomysql 连接池、参数化 SQL | MySQL 池与查询 |
| `libs/auth.py` | 多源认证、资料合成、用户保存 | 认证业务 |
| `libs/offline_auth.py` | 注册、登录、查询、改密 | 离线账户 |
| `libs/figura.py` | 在线认证、离线回退、会话、服务器同步 | Figura 业务 |
| `libs/utils.py` | UUID、bcrypt、时长、改名及关联 | 算法与资料业务 |
| `libs/skin.py` | Mojang/CSL、材质下载、缓存 | 皮肤与披风 |
| `libs/head.py` | 头像裁剪、透明叠加、缩放、默认皮肤 | PNG 头像 |

Rust 按用户补充要求改用 `config.toml` 与 `config.example.toml`，保留配置值语义和部署资源：`assets/alex.png`、`keys/`、`textures/`。核对可执行文件启动目录与相对路径，防止发布包找不到配置、公钥和默认皮肤。

数据库包含 `users`、`userlink`、`namelink`、`offline`、`figura`、`figura_server`、`skins`。保持字段、NULL、唯一约束、字符集和排序规则，不清库、不强制重注册。

`database.sql` 包含 DROP DATABASE/TABLE，禁止直接对业务库或测试实例运行。只提取必要建表定义，在现有 MySQL/MariaDB 实例的隔离测试库使用。不得下载或启动额外数据库服务器、容器或便携实例。

## 3. 必须保留的 HTTP 接口

| 方法 | 路径 |
| --- | --- |
| GET | `/` |
| GET | `/auth` |
| GET | `/auth/sessionserver/session/minecraft/hasJoined` |
| GET | `/auth/sessionserver/session/minecraft/profile/{uuid}` |
| GET | `/auth/api/users/profiles/minecraft/{username}` |
| GET | `/csl/{username}` |
| GET | `/textures/{texture_hash}` |
| GET | `/figura/hasJoined` |
| POST | `/figura/register` |
| POST | `/figura/refresh` |
| POST | `/figura/server` |
| POST | `/profile/linkprofile` |
| POST | `/profile/rename` |
| GET | `/profile/rstname/{uuid}` |
| GET | `/offline/check/{username}` |
| POST | `/offline/reg` |
| POST | `/offline/login` |
| POST | `/offline/chpasswd` |
| GET | `/check_token` |
| GET | `/head/{account}/{size}` |

同时确认 FastAPI 的 `/openapi.json`、`/docs`、`/redoc` 当前使用情况，并记录迁移决定。

兼容验收必须覆盖：

- 请求字段、参数大小写（如 `serverId`）、缺字段和类型错误；响应状态、中文消息、`detail` 嵌套、空 204、PNG 类型和 CORS。
- Bearer 缺失、错误格式和值不匹配；保持现有受保护路由范围，不直接采用框架默认错误体。
- `X-Forwarded-For` 首地址、直连 IP、离线注册请求体 IP 与 Figura 请求 IP 的区别。
- `servers` 顺序、enabled、四类来源 mojang/aihmc/blessingskin/elyby 的 URL、成功条件和回退规则。
- UUID 连字符处理与大小写；离线 UUID 使用 UTF-8 的 `OfflinePlayer:<name>` 直接 MD5，再设置版本及变体位，不能替换成带命名空间的 UUID v3。
- 历史 bcrypt 哈希能在 Rust 验密，新写入哈希可被 Python 验证；覆盖 Unicode 和长密码边界。
- textures value/signature 原样保留；当前公钥读取行为保留，不因有私钥配置就增加签名或轮换。
- Figura 在线来源优先、服务器同步命中、会话 IP/过期及现有未注册离线回退行为。
- 皮肤 default/slim、披风、缓存命名、`.json` 处理；头像基础层、透明叠加、最近邻缩放和 Alex 回退。

## 4. 拟定技术方案

使用单一 Cargo 应用，二进制名 `aihmc-auth`，不引入多 crate 框架。

| 能力 | 拟用方案 |
| --- | --- |
| 异步与 HTTP | Tokio、Axum、tower-http |
| 序列化 | Serde、serde_json、toml（用户明确要求） |
| 数据库 | SQLx MySQL 连接池、运行时参数化 SQL；普通编译不依赖在线数据库 |
| 上游请求 | reqwest，优先 Rustls，核验 HTTPS 和三平台支持 |
| 密码与 UUID | bcrypt、MD5、UUID 工具，按兼容样本验证 |
| 图片 | image，仅启用需要的格式能力 |
| 域名 | URL 与 Public Suffix List 解析，对照 tldextract 样本 |
| 日志 | tracing，不输出密码、Token、连接串 |

依赖版本在实施时查询官方元数据并实际解析验证；提交 `Cargo.lock`，`rust-toolchain.toml` 固定已验证工具链。不要用简单的最后两段域名切分代替公共后缀规则。

建议模块为 `config`、`state`、`http`、`db`、`auth`、`offline`、`figura`、`profile`、`skin`、`head`，保持 HTTP 协议适配与业务逻辑清晰分工，复用连接池和客户端，不增加无实际用途的抽象层。

## 5. 分阶段实施与验收

阶段严格顺序推进。每阶段所需自动验证通过后：更新计划和交接记录，检查 git status/diff，提交，推送 `aihmc-auth` 并核对远端，然后开始下一阶段。Actions 仍必须手动运行，不因阶段提交自动触发。

### 阶段 1：行为基线

交付 20 个业务路由的脱敏成功/失败样本、配置契约、数据库隔离数据集、可控上游测试服务和算法样本。比较两种实现时分别初始化隔离数据，避免重复写入污染结果。

验收：Python 基线可启动；路由主要分支有可重复结果；UUID、bcrypt、时长、域名/来源 URL 和 PNG 像素样本齐备。随机值比较格式和约束，时间相关使用受控时间。记录缺失的环境和实际未运行项目。

### 阶段 2：Rust 基础与手动编译工作流

建立 Cargo、配置、数据库池、共享客户端和启动；实现根路由、Token 及算法；加入第 6 节工作流。

验收：本地 Windows 格式检查、Clippy、测试和 release 编译通过。检查工作流仅有 workflow_dispatch。三平台编译需用户手动运行后记录；未运行记为 NOT RUN，不得冒称通过。本地必需验证通过后完成阶段提交推送，继续可自主执行的后续工作；手动 Actions 未运行单独列为待验，不作为人工等待阻塞条件，也不声明三平台已通过。

### 阶段 3：认证、账户、Figura 与资料

迁移多源认证、资料合成/持久化、离线注册登录改密、Figura 会话/同步以及名称和 UUID 关联。

验收：Windows 真实 HTTP 服务连接现有实例隔离库，通过来源顺序、禁用来源、空 204、历史密码、allow_offline、Figura 会话、同步命中、关联后查询等集成测试。受控上游测试标注为集成测试，不冒充真实上游验收。

### 阶段 4：皮肤、材质与头像

迁移 CSL、textures 解析、下载缓存、文件响应、头像与资源打包。

验收：HTTP 返回 PNG 可解码，像素/尺寸/透明度与样本一致；覆盖皮肤与披风、模型、缓存命中/缺失、默认头像、用户名和两种 UUID；不要求 PNG 压缩字节一致。

### 阶段 5：回归与交付

完成所有业务路由对照、发布包解压启动、部署和回滚说明，更新 README；保留 Python 基线提交用于回滚；覆盖完成后按用户要求将原 Python 版本完整归档至 `legacy/python/`，包含源码、原依赖、YAML 示例及独立运行说明，不归档真实配置或密钥。

验收：Windows 发布包在部署目录按文档启动并通过 HTTP/隔离库测试；经手动 Actions 得到三个平台编译、测试和解包冒烟结果，产物齐全。Linux 支持范围按 runner 的实际动态依赖记录，不声称支持所有发行版。

真实插件登录、真实账号上游验证和生产切换分别记录。需用户环境的人工项未做时记 `MANUAL/PENDING`，不得记 PASS，不阻止可自主完成的实施、记录及提交交付。不得自动生产部署。

## 6. GitHub Actions 设计

文件：后续实施时新增 `.github/workflows/rust.yml`。

唯一入口：

```yaml
on:
  workflow_dispatch:
```

不配置其他触发器，不在本次或后续开发期间擅自调用 `gh workflow run`、API dispatch 等执行入口。运行由用户手动发起。

| 标识 | Runner | Target | 归档名称 |
| --- | --- | --- | --- |
| linux-x64 | `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | `aihmc-auth-linux-x64.tar.gz` |
| linux-arm64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | `aihmc-auth-linux-arm64.tar.gz` |
| windows-x64 | `windows-2022` | `x86_64-pc-windows-msvc` | `aihmc-auth-windows-x64.zip` |

使用原生 runner 编译并运行各自测试，ARM64 不只进行交叉编译。首次落地核对仓库权限及 runner 可用性。

工作流要求：

1. `permissions: contents: read`，矩阵 `fail-fast: false`；固定工具链，缓存区分系统、架构、工具链、Cargo.lock。
2. 执行 fmt、Clippy、`cargo test --locked --target <target>`、`cargo build --locked --release --target <target>`；每条命令立即检查退出码。
3. 所有平台使用 `shell: pwsh`，执行前确认 PowerShell 可用；构建、测试、打包全程前台输出并 Tee-Object 记录，失败读取日志。
4. 解包后启动真实二进制并执行 HTTP 冒烟，测试夹具清理服务进程；不只验证文件存在。编译和测试命令本身不得后台运行。
5. 常规矩阵不要求生产数据库或真实上游密钥。数据库集成仅连接预配置的现有隔离实例，缺凭据明确记 NOT RUN；编译和独立测试不因此伪造集成通过。
6. 包含二进制、示例配置、Alex 资源、部署说明和必要目录；禁止真实配置、私钥、缓存、数据库凭据与 .codegraph。
7. 上传各平台包、校验和与日志，失败也保留日志；不自动发布 Release 或部署。Action 实施时选维护版本并固定 SHA。

所有构建与测试使用以下模式；步骤日志分别保存避免覆盖：

```powershell
& cargo build --locked --release --target $env:RUST_TARGET 2>&1 |
    Tee-Object -FilePath "build.log"
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath "build.log"
    exit $buildExitCode
}
```

PowerShell 打包脚本还需显式处理 cmdlet 失败，不能仅依赖 LASTEXITCODE。本地同样遵守此前台日志规范，仅使用 Windows target。

## 7. 已观察到的待判定项

以下仅为源码观察，需阶段 1 最小复现，不在本次顺带修复：

| 位置 | 观察 | 后续动作 |
| --- | --- | --- |
| `utils.link_server_profile` | 更新成功未返回成功值，路由按 falsy 判失败 | 核对数据库副作用与 HTTP 响应 |
| `skin.read_skin_data` | 在判空前访问 res[0] | 复现空记录 |
| `skin.request_skin_api` | 来源用 mojang，部分下载分支判断 official | 核对官方材质下载 |
| `auth.save_other_data` | INSERT IGNORE 与布尔执行结果决定冲突处理 | 构造用户名/UUID 冲突样本 |
| `auth.force_rename_to_aihmc` | 数据库持续失败会无限重试 | 记录故障，不以无限等待验收 |
| `head.py` / 依赖清单 | 使用 PIL，requirements 未列 Pillow | 记录 Python 基线环境所需依赖 |

已证实缺陷与兼容行为分开记录。必须变更行为时先说明最小范围并取得明确决定，禁止借迁移扩大范围。

## 8. 切换与回滚

备份原配置、密钥、数据库和纹理，不提交备份。Rust 先在不同端口和隔离库验证；获授权后才切换正式入口，不向生产 Python/Rust 同时发送同一写请求。

默认无表结构变更。回滚停止 Rust，恢复 Python 基线和入口，保留业务数据，验证 Rust 写入密码及缓存可由 Python 读取。确需结构变更时先审批并补回滚方案。

## 9. 进度

| 项目 | 状态 |
| --- | --- |
| 计划与源码盘点 | DONE |
| 阶段 1 行为基线 | DONE：Windows 18 项测试通过，含真实回环 HTTP 和现有 MariaDB 隔离库 |
| 阶段 2 基础与手动 CI | DONE：Windows fmt/Clippy、5 测试、release 包解压 HTTP 冒烟通过 |
| 阶段 3 认证与账户 | DONE：6 Rust 单元 + 6 真实 HTTP/MariaDB 集成；Windows release 包通过 |
| 阶段 4—5 实施 | NOT STARTED |
| 本地 Windows 编译测试 | NOT RUN |
| 三平台 Actions | NOT RUN，用户要求本次不执行，未来仅手动触发 |
| 真实插件及上游人工验收 | MANUAL/PENDING |

下一步执行阶段 4；Python 基线差异和验证细节见 `docs/PYTHON_BASELINE.md`。每阶段记录命令、工具链、架构、日志、实际通过/失败/未运行以及提交推送结果。

官方参考：

- GitHub runner：https://docs.github.com/en/actions/reference/runners/github-hosted-runners
- Rust target：https://doc.rust-lang.org/stable/rustc/platform-support.html

Runner 标签于本计划编写时查询官方资料；依赖具体版本在实施时实际核验。
