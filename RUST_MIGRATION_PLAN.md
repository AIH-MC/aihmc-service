# AIHMC 认证服务 Rust 迁移计划书

日期：2026-11-14。状态：R0 至 R4 本地自动验收通过，后续进度及提交状态见 `docs/RUST_MIGRATION_PROGRESS.md`。Actions 为非阻塞人工验收。

实施目标是将当前 Go 服务迁移至 Rust，保留现有业务行为与数据，将当前 Go 版本完整存档至独立目录，并配置现有 GitHub Actions 流程构建 Linux x64、Linux ARM64、Windows x64 发布包。本文将用户所述 Linux ARM 解释为 ARM64，不包含 ARMv7。用户已授权逐阶段验证、提交和推送；本地仅编译和测试 Windows x64。

最新执行规则：Actions 全部交由人工，Agent 禁止运行，不作为阶段或交付阻塞条件。工作流仅手动触发；本地 Windows 自动验收通过后即可提交推送并继续。Linux 构建、远端测试及产物验证未由人工提供结果时统一为非阻塞 MANUAL/PENDING，不声明三平台实际编译已通过。此规则覆盖本文原有远端 Actions 门槛。

## 1. 当前基线与范围

- 当前活动工程位于仓库根目录：`go.mod`、`go.sum`、`cmd/aihmc-api/`、`internal/`，Go 模块声明工具链为 1.26.0。
- 当前 `.github/workflows/build.yml` 在 `mc-auth` 推送、面向该分支的 PR 及手动触发时构建；只有 Linux amd64，包含二进制 `--help`、白名单打包、SHA256 和日志上传。
- 已有接口、协议向量、HTTP 对照和头像样本可复用，见 `docs/M0_COMPATIBILITY.md`、`docs/legacy-protocol-vectors.json`、`docs/go-http-baseline.json`、`docs/contract-comparison.json`、`tests/testdata/`。历史文档记录 39 项 PHP/Go HTTP 对照及十组头像参考；Rust 必须独立执行验证，不能继承历史 PASS。
- 旧 PHP 已在 `archive/php/`，保持原状。既有 Go 迁移计划和证据保留为历史记录，不改写成 Rust 验收结果。
- 现有分支为 `mc-auth`，跟踪 `origin/mc-auth`，远端为 `https://github.com/AIH-MC/aihmc-service.git`。

范围仅限语言迁移、必要的跨平台适配、Go 归档、Actions 和部署文档。保持域名、路由、MySQL/MariaDB schema、TOML 配置、UCenter、已有身份、令牌、RSA 密钥与材质目录。不得借机增加功能、更换认证算法、引入新数据库或重构旧 Go 代码。生产切换单列人工交接，不在编写计划时执行。

## 2. 现成 Rust 构建配置与 Actions 参考

### 2.1 直接复用现有 Rust 编译配置

本地相邻项目 `../aihmc-auth-backend/` 已存在以下配置，已读取核对：

| 来源 | 已有能力 | 本项目的使用方式 |
| --- | --- | --- |
| `.github/workflows/rust.yml` | 三平台原生 runner 矩阵、PowerShell、Rust 安装、缓存、构建脚本与产物上传 | 作为 Rust 构建模板，合并进本项目现有 `build.yml`，不新增重复发布工作流 |
| `rust-toolchain.toml` | 固定 1.95.0，minimal、rustfmt、clippy | 优先沿用现成配置，实施时实际核验安装及依赖解析 |
| `scripts/ci.ps1` | fmt、clippy、test、release、打包、解包、SHA256 和 smoke | 复用流程，按本项目名称、目录和启动参数适配 |
| `Cargo.toml` | Rust 2024、Tokio、Axum、SQLx MySQL、Reqwest、Rustls、Serde、TOML、图像等依赖 | 作为已存在选型参考，只引入当前业务确需的依赖 |

相邻项目不是本项目的实现或兼容性基准，不复制其业务逻辑、运行配置、凭据、无关资产和全部依赖。这里记录的版本来自现存配置，不声称为最新版本。配置交付包含三目标，但实际三平台构建结果由人工运行核验；若需修改，仅修复可复现的问题。

实施 R0 同步远端后确认：现有 `.github/workflows/rust.yml` 是 checkout `aihmc-auth` 分支的手动构建入口，予以保留，不影响其他分支。本计划的“唯一活动构建工作流”指 mc-auth 当前实现的默认构建由 build.yml 负责，不删除该历史跨分支手动入口，也不拿它的成功代替当前 Rust 实现验证。

适配时必须移除脚本中对 `aihmc-auth` 二进制名及其特有 `assets/` 的硬编码依赖。本项目二进制统一为 `aihmc-api`，Windows 为 `aihmc-api.exe`。脚本中每个原生命令立即检查退出码；调用 smoke 脚本后必须可靠传播失败，不能只输出打包成功消息。

### 2.2 参考 omni-morph 不同分支的 Actions 文件

按用户更正，本参考仅指 **Actions 配置文件处理方式**，不改变应用 TOML 配置。2026-11-14 通过 GitHub API 读取了以下分支的 `.github/workflows/fabric-build.yml`：

| 分支 | 观察结果 | 可复用约定 |
| --- | --- | --- |
| [main](https://github.com/AIH-Interactive/omni-morph/blob/main/.github/workflows/fabric-build.yml)、[dev](https://github.com/AIH-Interactive/omni-morph/blob/dev/.github/workflows/fabric-build.yml)、[acceleration-dev](https://github.com/AIH-Interactive/omni-morph/blob/acceleration-dev/.github/workflows/fabric-build.yml) | 手动触发、双 loader 矩阵、Java 25 | 分支并发取消、矩阵独立失败、前台日志、准确选择正式产物 |
| [1.20.1](https://github.com/AIH-Interactive/omni-morph/blob/1.20.1/.github/workflows/fabric-build.yml) | Forge/NeoForge，Java 17 目标配合 Java 21 构建 | 不同目标通过矩阵参数处理，明确工具链差异 |
| [1.21.1](https://github.com/AIH-Interactive/omni-morph/blob/1.21.1/.github/workflows/fabric-build.yml) | Fabric/NeoForge，Java 21 | 产物唯一性检查、SHA256 写入摘要 |
| [26.2](https://github.com/AIH-Interactive/omni-morph/blob/26.2/.github/workflows/fabric-build.yml)、[26.3](https://github.com/AIH-Interactive/omni-morph/blob/26.3/.github/workflows/fabric-build.yml) | Fabric/NeoForge，Java 25 | 对应分支和产物名称一致，正式产物保留 30 天、失败日志保留 14 天 |

本项目采用这些文件中的处理方式：`contents: read`、checkout 不持久化凭据、`pwsh`、按分支组织 concurrency、`cancel-in-progress: true`、`fail-fast: false`、明确超时、失败日志上传及正式产物白名单。concurrency 分组还包含工作流和 PR/分支标识，避免不同工作流互相取消。

参考仓库的 Java/Gradle 和 loader 不照搬。用户在 R1 更正要求提交不得自动触发 Actions，故本项目构建仅保留 workflow_dispatch，移除 push/PR；误触发运行手动停止。随后用户明确 Actions 全部交由人工、Agent 禁止运行，远端验收为非阻塞人工项。其 `upload-artifact` 使用 `archive: false` 直接交付正式文件的方式可用于本项目已有的压缩包及校验文件，实施时核对 Action 版本和参数支持；不得产生未预期的二次压缩或拿中间产物替代正式包。

## 3. Rust 实现方案与兼容边界

采用一个 Rust HTTP 服务，按现有职责组织 `config`、`httpapi`、`auth`、`store`、`ucenter`、`texture` 模块。优先复用现存 Rust 工具链与选型，不增加框架层、ORM 自动迁移或兼容包装层。MySQL 使用驱动连接系统已有实例，不安装数据库服务器；测试只写入隔离测试库。

### 必须保持的行为

| 模块 | 迁移及验证要求 |
| --- | --- |
| 配置与入口 | 保留 `--config`、默认 `config.toml`、`--check-config`；无实际配置时失败，不读取模板兜底；相对路径以配置文件目录为准；未知字段、时长、密钥和数据库检查与 Go 一致；`--help` 不依赖配置或数据库 |
| HTTP | 覆盖根路径、初始化、认证五接口、join/hasJoined、UUID/角色名/批量查询、两个角色名入口、头像、删除及未知路径；逐项保留方法、状态码、头、字段类型及空体语义 |
| 身份与令牌 | 保留 UID、profile_id、player_uuid、初始及刷新令牌格式；不改成 JWT，不重建身份，不新增过期策略；重复登录复用和共享 clientToken 的更新范围按现有契约 |
| 数据 | 复用现有六表及索引、字符集与排序规则，保持 NULL/空串差异、事务和删除范围；启动不建库、不改表、不执行破坏性 SQL |
| 协议与签名 | UCenter 保留 Base64(SHA256 十六进制文本)；材质保留 SHA1/RSA PKCS#1 v1.5；保存的 texture_data 原样签名，新生成材质序列化按现有固定向量逐字节比较 |
| 代理与授权 | 保留仅信任回环 TCP 对端的 X-Real-IP 规则；hasJoined 不新增 IP 限制；删除接口保留 `prarms.uid` 拼写及管理密钥边界 |
| 细节兼容 | unsigned 维持 PHP 字符串真值，尤其空串、`0`、`false`；不擅自改变未知路径、查询串和尾斜杠行为；204 响应为空体 |
| 材质与头像 | 保留上游顺序、回退、材质哈希和 URL、并发文件发布行为；复用样本验证 GD 7 位 alpha 舍入、模型判断和叠层；普通图像库缩放不能替代已验证算法 |
| 生命周期 | 支持监听失败、依赖失败、正常关闭与有界优雅退出；Linux 验证 SIGTERM，Windows 验证受支持的控制台停止方式，不新增 Windows 服务管理器 |

Rust JSON、URL 解码、浮点舍入、路径、文件权限和重命名行为不能直接假定等价。出现偏差时先用现有向量或 Go 实际输出复现，再做最小实现；产品行为变更另行确认。

## 4. Go 版本归档与最终目录

迁移开发期使用 `rust/` 临时目录，根目录 Go 工程持续可用。Rust 全量自动验证通过后再将 Rust 提升至根目录，把原 Go 工程归档至 `archive/go/`。不提前移动正在被运行进程引用的资源。

```text
Cargo.toml / Cargo.lock / rust-toolchain.toml
src/                         Rust 活动工程
tests/                       Rust 集成测试、共享 schema 与参考样本
scripts/                     本项目 CI 和冒烟脚本
.github/workflows/build.yml   唯一活动构建工作流
config.example.toml           当前配置模板
docs/                        当前部署说明、迁移进度和共享证据
archive/go/
  go.mod / go.sum
  cmd/ / internal/
  tests/                     Go 测试所需 schema 和样本
  docs/                      Go 基线与对应说明
  config.example.toml
  README.md
  GO_MIGRATION_PLAN.md
  .github/workflows/build.yml 原 Go 工作流快照，不会作为活动工作流运行
  ARCHIVE_MANIFEST.json       原相对路径、归档路径、SHA256、基线提交
archive/php/                 已有归档，保持原状
RUST_MIGRATION_PLAN.md
AGENT_HANDOFF.md
```

归档须保留 Go 运行和测试需要的受控源码、模块锁定文件、样本、模板、部署说明和原始 Action 快照；共享证据可在根目录继续保留一份。`internal/testutil/` 涉及的根目录定位及历史 PHP 引用路径在归档后实际核验，仅做必需的路径适配并记录差异，不能以旧根目录的测试结果代替归档验证。

归档前后生成文件清单和 SHA256 对照，核对无遗漏；归档后的 Go 模块可独立构建，默认根目录 Cargo 和 Actions 不扫描或发布归档代码。归档 README 写明基线提交、构建目录、版本及已验证的恢复方法。保留旧发布包供回退，源码归档不代表生产已切换。

禁止归档实际 `config.toml`、秘密、私钥、业务 SQL 数据、运行材质、构建缓存和含敏感信息的日志。真实资源仍留在受控运行目录；维护忽略规则，但不能仅依赖忽略规则判断安全。

## 5. 三平台 Actions 交付

复用已存在的 Rust 原生 runner 矩阵，避免为本任务另建交叉编译工具链：

| 平台 | runner（现有模板值） | Rust target | 包名 |
| --- | --- | --- | --- |
| Linux x64 | `ubuntu-24.04` | `x86_64-unknown-linux-musl` | `aihmc-service-linux-x64.tar.gz` |
| Linux ARM64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-musl` | `aihmc-service-linux-arm64.tar.gz` |
| Windows x64 | `windows-2022` | `x86_64-pc-windows-msvc` | `aihmc-service-windows-x64.zip` |

用户在 R1 明确要求 Linux 直接使用 musl，两个 GNU target 均替换为上表 musl target；原生 runner 安装 musl-tools，C 依赖和链接使用 musl-gcc。Actions 用 readelf 核对架构、无 INTERP 和无 NEEDED 动态依赖，并实际运行解包产物。Windows 核验 MSVC 运行库需求。依赖使用现有 Rustls 方案时验证系统证书和真实 HTTPS，不关闭证书验证解决问题。

流水线步骤：

1. checkout，安装项目固定工具链和 target；缓存键包含 OS、target、工具链和 Cargo.lock。
2. 使用提交的 Cargo.lock，执行 `cargo fmt --all -- --check`、目标平台 clippy、测试及 `cargo build --locked --release --target <target>`。检查项失败即停止当前 job，其他矩阵项继续运行。
3. 仅将对应目标的 `aihmc-api[.exe]`、配置模板、README 和部署说明加入 staging，不复制整个仓库，不带历史源码或秘密。
4. Linux 保留可执行权限并 tar.gz，Windows zip；生成 SHA256；解包正式产物后执行冒烟验证，避免只验证 target 目录中的裸二进制。
5. 冒烟至少覆盖 `--help`、缺配置的非零退出，以及使用临时非秘密配置和受控依赖的实际 HTTP 启动/停止。`--check-config` 涉及数据库时归入有真实隔离库的集成验证，不通过跳过数据库来冒充成功。
6. 上传三套明确命名的包和 SHA256，摘要记录目标、工具链、提交与校验值；产物缺失视为失败。成功包保留 30 天，失败日志保留 14 天，日志名称包含平台避免冲突。

公共 runner 无法访问现有数据库时，三平台 CI 必须通过纯逻辑及不需要数据库的真实进程检查；数据库集成另在可访问现有实例的环境执行并记录证据。跳过的数据库用例必须显式列出，不能把矩阵绿色当作全业务验收。

所有编译、测试、打包命令在前台 PowerShell 输出到 `build.log`，立即捕获并检查退出码，失败读取日志。例如：

```powershell
& cargo build --locked --release --target x86_64-pc-windows-msvc 2>&1 |
    Tee-Object -FilePath 'build.log'
$buildExitCode = $LASTEXITCODE
if ($buildExitCode -ne 0) {
    Get-Content -LiteralPath 'build.log'
    exit $buildExitCode
}
```

PowerShell cmdlet 的打包失败使用终止错误传播；不能对 cmdlet 错误错误地依赖上一次原生命令的 `$LASTEXITCODE`。fmt、clippy、测试、编译、打包和 smoke 日志分别保存，防止后一步覆盖失败证据。

## 6. 实施阶段与验收

阶段按顺序执行，不将多个完成阶段积压到最后一次提交。迁移实施已获授权，生产切换仍按人工交接处理。

| 阶段 | 工作与交付 | 自动验收要求 |
| --- | --- | --- |
| R0 固定 Go 基线与配置复用 | 记录源提交、接口和数据副作用、归档清单；复验现有向量；核验现成 Rust 编译脚本与 omni-morph Actions 约定 | Go 基线可重复；旧测试跳过项明确；配置复用差异清单齐备 |
| R1 Rust 入口与最小构建链 | 在 `rust/` 建立工程、锁定依赖，迁移配置、日志、CLI、数据库连接、生命周期；接入仅手动触发的三平台 job | Windows release、CLI 冒烟、配置正反例和路径测试通过；Actions/Linux 为非阻塞 MANUAL/PENDING |
| R2 数据及协议 | 六表访问与事务、UCenter、令牌和身份、RSA 与材质序列化 | 现有数据库实例内独立测试库集成通过；固定向量、事务回滚和 Go/Rust 双向签名验证通过 |
| R3 认证与会话 HTTP | 完整认证、角色、会话、默认路由和错误行为 | 受控上游下 Go/Rust 完整请求序列对照；身份、令牌互认、并发及最终数据库状态一致 |
| R4 材质头像与删除 | 下载、回退、存储、头像、授权删除 | 现有十组头像逐像素对照、新材质逐字节对照、上游失败、并发发布、删除范围及回滚通过 |
| R5 归档与最终流水线 | 归档 Go、Rust 提升根目录、消除临时构建路径、适配打包及部署说明 | 归档清单核验、Go 归档独立 Windows 构建、Rust 根目录 Windows 全量回归和包解包冒烟；三平台 Actions 与远端包校验交由人工，非阻塞 MANUAL/PENDING |
| R6 交付与生产交接 | 记录 Actions run、提交、包、日志和验证边界，更新交接手册，提供切换与回退步骤 | 自动证据齐备；未执行人工项为非阻塞 MANUAL/PENDING，不宣称上线或真实用户链路已通过 |

每阶段本地 Windows 自动验收后：更新进度记录 → 检查 `git status`、`git diff`、`git diff --check` → 在 `mc-auth` 提交 → 推送同名远端分支 → 确认成功 → 开始下一阶段。保留已有修改，不强推、不自动合并 main、不引入新分支前缀。R5 必须实际验证提升后的根目录，不能用临时目录构建结果替代。Actions 不运行、不等待，不阻塞阶段与交付。

## 7. 真实行为验证与上线交接

Go 与 Rust 在相同初始隔离数据、相同受控上游下分别运行请求序列，比较状态码、响应头、JSON 字段类型、空体、数据库最终状态及材质文件。随机值只允许按事先定义的字段归一化；固定材质和签名输入不归一化。不能对同一个测试状态重复执行两遍写请求作为对照。

必要回归包括：首次和重复登录、错误密码、中文及大小写角色名、requestUser、缺失 clientToken、刷新后令牌、失效与退出、错误 selectedProfile、join/hasJoined、记录清理、重启后旧令牌、Go 读 Rust 写入数据和 Rust 读 Go 数据。不在生产上执行删除验收。

真实 UCenter 账号、启动器登录、Minecraft 进服、客户端材质、生产网关、目标部署和生产回退等需人工配合的项目，未执行记录 `MANUAL/PENDING`，用户要求跳过才记 `SKIPPED`；均不阻塞自主实施、自动验证、提交、推送和交付。服务侧可自主进行的真实数据库及受控 HTTP 验证仍需完成。

生产交接沿用既有网关和静态材质服务，使用同一数据库、密钥和资源目录；切换前备份，收口旧进程新请求并等待在途写入，再整体切换认证路由。回退到受控的原 Go 发布包，常规回退不恢复旧数据库快照，以免覆盖切换后的有效写入。Go/Rust 双向读取通过才声明数据层回退已验证。

本服务不引入 Qt。若客户端联调涉及 Qt，只允许真实 Windows 窗口验证，禁止 offscreen。

## 8. 最终完成标准与当前状态

- [ ] Rust 已替换全部现有受支持服务路径，协议、状态与数据副作用通过对照。
- [ ] 既有配置、数据库、身份、令牌、密钥、材质继续可用，无业务 schema 迁移。
- [ ] Go 完整存档于 `archive/go/`，归档可独立构建且不含真实秘密。
- [ ] 根目录 Rust 的三平台手动 Action 配置交付，Windows 本地实际构建成功；Actions/Linux 结果非阻塞 MANUAL/PENDING。
- [ ] Windows 正式发布包解包冒烟通过，SHA256、平台、提交与日志一致；两个 Linux 发布包由人工运行验收，非阻塞 MANUAL/PENDING。
- [ ] 部署回退说明、人工项状态及最终 `AGENT_HANDOFF.md` 更新完成。
- [ ] 各阶段提交已推送并确认，交接说明待人工运行 Actions 的目标分支及步骤。

实施授权及逐阶段证据见 `docs/RUST_MIGRATION_PROGRESS.md`。既有运行配置处理方式保持不变。
