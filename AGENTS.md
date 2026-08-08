# patchyBox · 项目简报（AGENTS.md）

> 本文件在会话打开 `G:\workspace\patchyBox` 时自动注入，先读它再动手。
> 设计阶段已完成（2026-08-02）；**M0/M1 已完成（2026-08-02）**，当前目标：**M2 第二批 I（HTTP/WS 调试、SQLite 数据库、hosts 修改 + 轻量工具）**。

## 项目是什么

基于 **Tauri 2** 的桌面工具箱 **patchyBox**，集成常用开发与效率工具。UI 方向已定稿：**明净浅色 · 内容优先**（macOS 式侧栏 + 大卡片网格），原型见 `sketches/002-clean-light/`。

## 交付路线（两批次）

- **第一批（当前目标）**：整体 UI 框架 + 后端框架 + **8 个常用文本小工具**（JSON 格式化 / 时间戳转换 / Base64 编解码 / URL 编解码 / 字符统计 / 文本对比 / Markdown 预览 / 正则测试）
- **第二批**：复杂工具——HTTP/WS 调试、数据库（MySQL/PG/SQLite）、本地 hosts 修改、DNS 管理（阿里/腾讯/CF）、SSH 工具

**铁律：第二批的工具形态（大工作区、长连接、凭据、提权）必须在第一批框架里预留抽象，第二批只填实现不改框架。** 第一批代码对第二批"只增不改"。

## 已定技术决策（详见 docs/01-tech-stack.md）

- 容器：Tauri 2.11（Rust）+ 官方插件：clipboard-manager / global-shortcut / store / sql / single-instance / autostart / notification / window-state / updater / dialog / fs / opener
- 前端：**Vue 3.5 + TypeScript + Vite + Tailwind CSS 4 + Pinia** + fuse.js（搜索）+ vue-i18n（zh-CN 默认）
- 不引入重型组件库，UI 自研，tokens 以根目录 `DESIGN.md` 为**单一事实源**
- 明确禁用 `tauri-plugin-shell`
- 第二批技术预备（**第一批不引入依赖**，只留架构边界）：reqwest + tokio-tungstenite（HTTP/WS）、sqlx（DB 三方言）、russh 或 ssh2（SSH，M3 再定）、tauri-plugin-stronghold（凭据加密）、hosts 按需提权助手（UAC）

## 架构核心（详见 docs/02-architecture.md）

- **插件模式（2026-08-08 起，前后端同构）**：前端 `src/plugins/<id>/`（manifest 自注册 + 私有 contracts.ts/ipc.ts + 组件，插件间禁止互相 import）；公共能力走 `src/core/ui/`（组件）与 `src/core/ipc/`（框架命令 + invokeCommand 基础设施）；**Rust 侧 `src-tauri/src/plugins/<id>/` 一律目录结构**（mod.rs 门面 + models.rs + 能力子模块；命令 + State 由 `register(builder)` 自注册，启动初始化走 `init(app)`），lib.rs 仅 register/init 各一行链式装配，插件清单见 `plugins/mod.rs`
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
5. 第二批凭据：默认 **stronghold**（备选 Windows Credential Manager / keyring）
6. hosts 提权：默认**按需提权助手**（UAC 最小授权），应用本体不常驻管理员

## 下一步：M3 第二批 II（新会话任务）

**M2 已全部完成（2026-08-02，共 17 个工具，验收全绿）**：轻量工具 4 + 剪贴板历史 + 快捷键改键 + HTTP/WS 调试 + SQLite 数据库 + hosts 修改。

M3 按 docs/02-architecture.md §10 推进：

1. **数据库扩展**：MySQL/PG 连接（复用 `modules/db`，sqlx 三方言 Adapter）
2. **DNS 管理**：✅ 已完成（2026-08-08，见下方实现记录）
3. **SSH 工具**：russh 或 ssh2（M3 再定），`modules/ssh`
4. **凭据加密**：tauri-plugin-stronghold（连接凭据落盘保护）——**DNS 插件密钥当前明文存 dns.db，接入 stronghold 时迁移**
5. **i18n / 发布准备**：英文语言包、自动更新、代码签名

### M3 已完成记录（2026-08-08）

- **DNS 工具**：`plugins/dns`（Rust `src-tauri/src/plugins/dns/`：mod.rs 门面 + models.rs + query.rs + alidns.rs + dnspod.rs）。DNS 查询用 hickory-resolver 0.26（feature `tokio`+`system-config`；**API 与旧版差异大：类型是 `TokioResolver`/`Resolver<TokioRuntimeProvider>` Builder 模式、`ResolverConfig::from_parts` 3 参、`NameServerConfig::udp(ip)`、Record 的 name/ttl/data 是公开字段、`Lookup::answers()`、TXT 用 `.txt_data` 字段**）；云解析：阿里云走 aliyun-openapi-core-rust-sdk 1.1（同参考项目 DnsAnalysisTools），DNSPod 走 dnsapi.cn Token API（POST 表单，零新依赖，参考项目用的是腾讯云 TC3 SDK）。8 命令 dns_query/dns_domains/dns_records/dns_add_record/dns_update_record/dns_delete_record/dns_config_get/dns_config_set 全量入库；add/update 用 payload 结构体打包（规避 clippy too_many_arguments）；密钥存 dns.db（PluginDb）明文。前端 plugins/dns/：三页签（DNS 查询/解析管理/密钥设置），查询面板多服务器对比 + 自定义服务器，解析管理两段式删除确认 + 行内表单 + 分页；recordTypeBadgeClass 色标在 useDns.ts。

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
- 所有命令在 git-bash 执行；Windows 环境
- 改 DESIGN.md 后跑 `npx -y -p @google/design.md designmd lint DESIGN.md` 校验
