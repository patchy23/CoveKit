# patchyBox · 项目简报（AGENTS.md）

> 本文件在会话打开 `G:\workspace\patchyBox` 时自动注入，先读它再动手。
> 设计阶段已完成（2026-08-02）；**M0–M3 已完成**，当前目标：**M4 发布准备**。

## 项目是什么

基于 **Tauri 2** 的桌面工具箱 **patchyBox**，集成常用开发与效率工具。UI 方向已定稿：**明净浅色 · 内容优先**（macOS 式侧栏 + 大卡片网格），原型见 `sketches/002-clean-light/`。

## 交付路线（两批次）

- **第一批（已完成）**：整体 UI 框架 + 后端框架 + 常用文本工具
- **第二批**：复杂工具——HTTP/WS 调试、数据库（MySQL/PG/SQLite）、本地 hosts 修改、DNS 管理（阿里/腾讯/CF）、SSH 工具

**铁律：第二批的工具形态（大工作区、长连接、凭据、提权）必须在第一批框架里预留抽象，第二批只填实现不改框架。** 第一批代码对第二批"只增不改"。

## 已定技术决策（详见 docs/01-tech-stack.md）

- 容器：Tauri 2.11（Rust）+ 官方插件：clipboard-manager / global-shortcut / store / sql / single-instance / autostart / notification / window-state / updater / dialog / fs / opener
- 前端：**Vue 3.5 + TypeScript + Vite + Tailwind CSS 4 + Pinia** + fuse.js（搜索）+ vue-i18n（zh-CN 默认）
- 公共 UI 采用 **shadcn-vue 源码模式 + Reka UI 无样式原语**，业务仅从 `src/core/ui` 使用 `Ui*`；tokens 仍以根目录 `DESIGN.md` 为**单一事实源**，禁止引入第三方默认皮肤
- 明确禁用 `tauri-plugin-shell`
- 第二批实际实现：reqwest + tokio-tungstenite（HTTP/WS）、多驱动数据库 Adapter、russh（SSH）、系统 keyring + AES-256-GCM（凭据）、hosts 按需提权助手（UAC）；Stronghold 因调试态快照性能与迁移成本已弃用

## 架构核心（详见 docs/02-architecture.md）

- **插件模式（2026-08-08 起，前后端同构）**：前端 `src/plugins/<id>/`（manifest 自注册 + 私有 contracts.ts/ipc.ts + 组件，插件间禁止互相 import）；公共能力走 `src/core/ui/`（组件）与 `src/core/ipc/`（框架命令 + invokeCommand 基础设施）；**Rust 侧 `src-tauri/src/plugins/<id>/` 一律目录结构**（mod.rs 门面 + models.rs + 能力子模块；命令清单由插件私有 `invoke_handler` 聚合，State 由 `register(builder)` 自注册，启动初始化走 `init(app)`）；应用级 `Builder::invoke_handler` 只能设置一次（后调用会覆盖前者），由 `plugins/mod.rs` 按前缀分派到插件 handler；lib.rs 仅保留一个总 handler 与 register/init 各一行链式装配
- **框架与插件分离**：`src-tauri/src/framework/`（设置存储/全局快捷键/窗口/命令入库/数据管理）是基建不属于插件；**无实际前端引用的插件必须删除**（color/clipboard 已删，设置弹窗剪贴板区块已清）
- **IPC 接口入库**（`framework/ipc_registry.rs`）：插件 register() 登记命令（名称+中文说明），启动校验全局唯一（重复即 panic）；框架命令 `framework_commands` 可查全量清单
- **数据库管理规则**（`framework/store.rs`）：插件数据文件统一 `app_data_dir/<plugin>.db`（`plugin_db_path`）；表结构走 `PRAGMA user_version` 顺序迁移（`migrate`，只追加）；**本地库统一骨架 `PluginDb`**（连接生命周期 + 锁 + 迁移，插件只写业务 SQL）；连接型 sqlx（三方言 M3）不套用
- **工具注册表**（`src/core/registry/`）：工具目录自注册，新增工具 = 建目录 + 注册一行，框架零改动
- **presentation 双载体**：`workspace`（**多页签工作区，2026-08-02 用户决策：第一批起全部工具以子页面打开**，实现见 `src/features/workspace/ToolWorkspace.vue`）/ `modal`（轻量弹窗，备用载体）
- **Rust 插件化**：`src-tauri/src/plugins/`（与前端 plugins 同名同边界；settings/clipboard/color 基础，http_ws/api/db/hosts 业务；后续 dns/ssh/secrets 同目录），Adapter 模式（Provider trait）是第二批骨架
- **IPC 契约插件化**：框架契约 `src/core/ipc/contracts.ts`（窗口/设置/剪贴板/取色）；插件契约在各自 `src/plugins/<id>/contracts.ts`（与 Rust serde 同步，互不影响）
- **工具级设置**：manifest 声明 `settingsSchema`，框架自动渲染设置表单并存 `settings.tools[id]`

## 文档地图

| 文件                                               | 内容                                                             |
| -------------------------------------------------- | ---------------------------------------------------------------- |
| `DESIGN.md`                                        | 设计 tokens：28 色 / 18 组件变体，已过 `designmd lint`（0 错误） |
| `docs/01-tech-stack.md`                            | 技术选型 + 第二批技术预备评估                                    |
| `docs/02-architecture.md`                          | 两批次详细设计：架构 / 注册表 / IPC / 安全 / 路线图 M0–M4        |
| `docs/03-plugin-development.md`                    | **插件开发规则 v1（2026-08-08）：IPC 接口入库 / 数据库管理 / 复杂工具架构 / 质量门槛 / 新增插件 Check-list** |
| `sketches/002-clean-light/`                        | 已采纳方向的**可交互原型**，开发验收视觉参照                     |
| `sketches/001-command-dark/` `003-glass-launcher/` | 未采纳方向，留档勿删                                             |

## 关键设计约束（来自 DESIGN.md，务必遵守）

- 交互驱动色 `tertiary #F0562C`，但**小号白字按钮必须用 `tertiary-strong #C2410C`**（WCAG AA）
- 选中态文字（nav/chip active）用 tertiary-strong；HOT 标签用 `success-strong #067647`
- 深色模式走 `-dark` token 变体，不新造色值；卡片网格 `auto-fill minmax(228px,1fr)`

## 工程约束（第一批验收红线）

**可读性**：组件 < 300 行；逻辑抽纯函数（`tools/<id>/useXxx.ts`）；IPC 出入参只在 contracts.ts 出现一次；ESLint 9 + Prettier + rustfmt + `clippy -D warnings` 全绿才合入。
**字体规范**：字号一律用语义 token（`text-h1/text-brand/text-card-title/text-h2/text-body/text-body-sm/text-caption/text-label-caps/text-display`，定义在 `src/assets/styles/main.css` @theme），禁止 arbitrary `text-[*px]`；字体族用 `--font-sans`（Inter Variable 本地打包）/ `--font-mono`（等宽）。
**可扩展性**：新增工具 = `src/plugins/<id>/` 目录（manifest + 私有契约/封装）+ `plugins/index.ts` 一行；新增 Rust 插件 = `src-tauri/src/plugins/<id>.rs` + `mod.rs`/lib.rs 各一行；新增厂商/数据库 = adapter；升级第二批时第一批代码只增不改。

## 开放问题（默认值已定，开工前可与用户确认）

1. 前端框架：默认 **Vue 3**（React 需在 M0 前提出，改动面见技术选型 §2）
2. 应用 identifier：默认 `com.patchy23.patchybox`（tauri.conf.json）
3. 关窗行为：默认**最小化到托盘**
4. 剪贴板隐私：默认仅忽略列表，不做启发式过滤
5. 第二批凭据：已定 **系统 keyring 保存主密钥 + AES-256-GCM 加密凭据文件**；Stronghold 已弃用
6. hosts 提权：默认**按需提权助手**（UAC 最小授权），应用本体不常驻管理员

## 下一步：M4 发布准备

截至 2026-08-24：

1. **数据库工作台主体已完成**：MySQL、PostgreSQL、SQLite、Redis、Oracle、PolarDB、Vastbase、Kingbase 已接入；达梦仅保留 UI 入口。
2. **DNS 已完成**：DNS 查询、阿里云、腾讯云 DNSPod、Cloudflare 均可用。阿里/腾讯可选择 Vault AccessKey 对，Cloudflare 可选择 Vault API Token；三者均保留原手工输入与 `dns.db` 存储。
3. **SSH 主体已完成**：russh 会话、终端、SFTP/远程编辑、监控、进程、systemd、Docker 管理均已落地；连接可选择公共 Vault 的用户名密码/SSH 私钥，也保留插件私有 AES 手工凭据。
4. **公共 Vault 已完成**：系统 keyring 保存主密钥，AES-256-GCM 保存凭据本体，并支持 Argon2id 加密备份；数据库、SSH、DNS 均已接入可选引用；删除前汇总 DNS/SSH 引用，失效与类型不匹配由前后端双重提示。
5. **M4 发布基建已完成**：核心界面中英文切换、Tauri Updater、GitHub 标签发布与签名参数已接入；待完成发布候选版验证，真实签名依赖 GitHub secrets 与平台证书。

### M3 进展记录（2026-08-08 起）

- **数据库工具**：`plugins/database` 已演进为多连接工作台，支持对象树、SQL 编辑/格式化/语句级执行、查询取消、数据与结构页签、建库建表及表维护；原生驱动与 agent 侧车统一走 Adapter 边界。
- **SSH 工具**：`plugins/ssh` 使用 russh，覆盖连接配置、主机密钥校验、PTY 终端、SFTP、远程编辑、文件传输、资源监控、进程、systemd 服务和 Docker 管理；认证支持公共 Vault 可选引用与原手工输入双路径；真实服务器集成测试默认 `#[ignore]`，通过 `SSH_TEST_*` 环境变量手动运行。
- **公共 Vault**：`framework/vault` 提供凭证 CRUD、脱敏摘要、导入导出和加密备份；主密钥优先存系统 keyring，不可用时回退本地密钥文件并告警。数据库旧 Stronghold/AES 存储已迁移到公共凭证命名空间。

- **DNS 工具**：`plugins/dns`（Rust `src-tauri/src/plugins/dns/`：mod.rs 门面 + models.rs + query.rs + alidns.rs + dnspod.rs + cloudflare.rs）。DNS 查询用 hickory-resolver 0.26；云解析覆盖阿里云 OpenAPI、腾讯云 DNSPod API 3.0 与 Cloudflare API v4。Cloudflare 使用推荐的 Bearer API Token，Zone 列表完整分页，记录管理按 Zone ID 执行。8 个 DNS 命令全量入库；配置支持 Vault 或手工输入双路径，阿里/腾讯使用 AccessKey 对，Cloudflare 使用 API Token，手工值继续存 `dns.db`。前端保持查询/解析管理/密钥设置三页签，并支持三平台切换、搜索、分页、行内增改与两段式删除确认。

### M2 已完成记录（2026-08-02）

- 轻量工具：随机密码（usePassword）/ 哈希（useHash，MD5+WebCrypto SHA）/ 颜色选择器（useColor + color_pick_screen）/ 二维码（qrcode 库）
- 剪贴板历史：前端页签（搜索/置顶/复制/删除/清空 + 3s 轮询），后端 M1 就绪
- 快捷键改键：Rust `modules/settings.rs` register_hotkey（读 settings.globalHotkey + 动态切换），设置页预设 select
- **HTTP/WS 调试**：`modules/http_ws.rs`（reqwest 0.12 + tokio-tungstenite 0.24 + tokio + futures-util）；6 命令 http_request/ws_connect/ws_send/ws_recv/ws_close/ws_sessions；前端 tools/http-ws（HttpPanel/WsPanel 双面板，JSON 响应高亮，WS 300ms 轮询拉取）；WS 会话 = id → 后台读写任务（tokio::select）+ 消息队列（上限 500 条）；WS 连接支持自定义请求头（token 认证）
- **SQLite 数据库**：`modules/db.rs`（sqlx 0.8 runtime-tokio+sqlite，M3 接 MySQL/PG 加 ConnectOptions）；5 命令 db_open/db_close/db_tables/db_execute/db_query_table；前端 tools/sqlite（表列表 + SQL 编辑 + 结果表格，NULL/blob 特殊显示，表名转义防注入）
- **hosts 修改**：`modules/hosts.rs`（std::process + PowerShell Start-Process -Verb RunAs UAC 最小授权；备份 hosts.bak-<时间戳> + 临时文件/ps1 中转，仅备份→覆盖一步提权）；2 命令 hosts_read/hosts_save；前端 tools/hosts（LineNumberTextarea 编辑 + 实时语法校验 + 错误明细 + 保存前校验拦截）
- 依赖备忘：tokio 需显式声明（tauri 不 re-export crate）；reqwest RequestBuilder 无 map_err（send 时出错）；tokio-tungstenite 0.24 的 Message 在 `tungstenite::` 下；sqlx 0.8 需显式 import `sqlx::Column`（name 方法），NULL 单元格用 `try_get::<Option<String>>` 兜底；MutexGuard 不能跨 await（先 take 出值再 await）

### M1 环境备忘补充

- 全局快捷键 Ctrl+Shift+Space 在本机被占用时降级告警（`[shortcut] 注册失败`），设置页改键功能 M2 接入（Rust 侧注册读取 settings.globalHotkey）
- 剪贴板历史 db：`%APPDATA%/com.patchy23.patchybox/clipboard.db`（rusqlite bundled）；历史上限清理未实现（M2 接 settings.clipboard.historyLimit）

### M0 环境备忘（Windows）

- **pnpm 命令**：系统 corepack shim 损坏（`G:\d\base\nodejs\pnpm` 报 MODULE_NOT_FOUND），需先 `export PATH="/c/Users/patchy/AppData/Roaming/npm:$PATH"`（npm 全局 pnpm 11.18.0）再执行 pnpm
- **pnpm 11 配置**：settings 在 `pnpm-workspace.yaml`（`allowBuilds: { esbuild: true }`），package.json 的 `pnpm` 字段已废弃不再读取

## 约定

- 文档与注释中文；代码标识符英文；提交信息用 conventional commits 带模块作用域（如 `fix(http-ws): 保存对话框`；跨模块逗号分隔；框架/基建用 `core`）
- 提交要求（所有项目通用）：Conventional Commits `类型(scope): 中文描述`；标题一行总概括，body 按代码增删改分条、内容具体（禁"xx产品化"式模糊概括），不写文件级细节/架构性质；类型按实质（搬移 refactor/隐患 fix/新机制 feat/测试 test）、scope 按真实改动模块；禁破折号与评审编号。署名：每会话首次提交前与用户明确（用户名+邮箱），无本地 git 配置则添加 local，会话内统一。提交时机：默认自动提交（除非明确说不提交）；复杂模块开发前先提交基线；任务尽量一次提交、大任务按阶段；只 commit 不 push
- 所有命令在 git-bash 执行；Windows 环境
- 改 DESIGN.md 后跑 `npx -y -p @google/design.md designmd lint DESIGN.md` 校验
