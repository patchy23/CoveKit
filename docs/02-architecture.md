# 详细设计 · patchyBox 桌面工具箱

> 视觉方向：**明净浅色 · 内容优先**（已采纳，原型 `sketches/002-clean-light/`）；设计规范：`DESIGN.md`（已过 lint，0 错误）。
> 技术栈：Tauri 2.11 + Vue 3.5 + TS + Tailwind 4 + Pinia（详见 `01-tech-stack.md`）。

---

## 0. 交付路线（两批次）

| 批次 | 内容 | 目标 |
|------|------|------|
| **第一批** | 整体 UI 框架 + 后端框架 + **8 个常用文本小工具** | 框架稳定、可读可扩展，工具即插即用 |
| **第二批** | 复杂工具：HTTP/WS 调试、数据库（MySQL/PG/SQLite）、本地 hosts 修改、DNS 管理（阿里/腾讯/CF）、SSH | 连接型 / 系统级 / 云 API 型工具 |

**关键原则：第二批的工具形态（大工作区、长连接、凭据、提权）必须在第一批的框架抽象里预留**，第二批只做"填实现"，不改框架。

---

## 1. 总体架构

```
┌────────────────────────────── 前端 (src/) ──────────────────────────────┐
│  features/  Sidebar · TopBar · ToolGrid · ToolList · ToolModal ·        │
│             ToolWorkspace(第二批) · Toast · Settings                    │
│        ▲ 使用                                                           │
│  core/    toolRegistry(注册表) · presentation(modal/workspace 路由)      │
│           search(模糊搜索) · ipc(类型安全调用) · sessions(会话,第二批预留) │
│        ▲ 读写                                                           │
│  stores/  tools · settings · favorites · clipboard · ui   (Pinia)       │
│        ▲ 懒加载                                                        │
│  tools/   json-formatter · ts-converter · …  (目录即工具, 自注册)         │
└──────────────────────────────────┬──────────────────────────────────────┘
                                   │ invoke() / listen()   (core/ipc 类型安全封装)
┌────────────────────────────── Rust (src-tauri/) ───────────────────────┐
│  framework/  commands 装配 · 插件注册 · 托盘 · 全局快捷键 · 单实例        │
│  modules/    settings · clipboard · color · …  (第一批)                 │
│              http · ws · db · hosts · dns · ssh · secrets (第二批预留)   │
│  services/   剪贴板轮询 · 提权助手 · 凭据加密(stronghold)                │
└──────────────────────────────────────────────────────────────────────────┘
```

**核心数据流（第一批示例：用户打开「JSON 格式化」）：**

```
ToolGrid 点击卡片 → ToolModal 挂载
  → dynamic import 加载工具组件（首次，注册表 component() 工厂）
  → 工具内纯函数处理文本（无需 IPC，全部前端完成）
  → 需要系统能力时（第二批）→ core/ipc → invoke → Rust module
```

**框架与工具的边界（第一批就定死）：**
- 框架（`core/` + `features/` + `src-tauri/framework/`）负责：注册、路由、状态、主题、搜索、持久化、IPC 装配；
- 工具（`tools/` + `src-tauri/modules/`）只做：自己的 UI + 自己的逻辑，**禁止**反向依赖框架内部实现；
- 工具通过注册表声明自己需要什么（presentation、settings 字段、IPC 命令），框架按声明提供服务。

---

## 2. 目录结构

```
patchyBox/
├── DESIGN.md / AGENTS.md / docs/ / sketches/
├── index.html / package.json / vite.config.ts
├── src/
│   ├── main.ts / App.vue            # App: 布局骨架 + 工具路由(modal/workspace)
│   ├── assets/styles/main.css       # Tailwind 4 @theme（对接 DESIGN.md tokens）
│   ├── core/
│   │   ├── ipc/ipc.ts               # invoke/listen 封装 + 错误归一化
│   │   ├── ipc/contracts.ts         # IPC 类型契约（唯一事实源，与 Rust serde 同步）
│   │   ├── registry/toolRegistry.ts # 工具注册表（核心）
│   │   ├── registry/types.ts        # ToolManifest 等接口（含 presentation）
│   │   ├── presentation/            # modal/workspace 两种载体路由（workspace 第一批仅骨架）
│   │   ├── search/fuzzy.ts          # fuse.js 封装 + 高亮
│   │   ├── hotkeys/hotkeys.ts       # 应用内快捷键
│   │   └── sessions/                # 会话模型（第二批预留：DB/SSH/WS 连接句柄）
│   ├── features/
│   │   ├── sidebar/ topbar/ grid/ modal/ workspace/ settings/ ui/
│   ├── tools/                       # 第一批 8 个文本工具（每个目录独立可测）
│   │   ├── json-formatter/{index.vue, useFormat.ts}
│   │   ├── ts-converter/ base64/ url-codec/ char-stat/ text-diff/ markdown-preview/ regex-tester/
│   ├── stores/                      # tools settings favorites clipboard ui
│   └── locales/zh-CN.ts
├── src-tauri/
│   ├── Cargo.toml / tauri.conf.json / capabilities/default.json / icons/
│   ├── src/
│   │   ├── main.rs                  # 入口：框架装配（plugins + commands + tray + 单实例）
│   │   ├── lib.rs                   # Builder 装配（供测试复用）
│   │   ├── framework/               # 框架层：命令注册宏、事件封装、窗口管理
│   │   ├── modules/                 # 业务模块（第一批）
│   │   │   ├── settings.rs clipboard.rs color.rs
│   │   ├── services/                # 剪贴板轮询、提权助手、凭据加密
│   │   └── (第二批预留模块说明见 §4.3)
│   └── tests/
└── e2e/                             # Playwright（M4 可选）
```

---

## 3. 数据模型

```ts
// core/registry/types.ts
export type CategoryId = 'dev' | 'text' | 'image' | 'net' | 'sys';
export type Presentation = 'modal' | 'workspace';   // 第二批工具用 workspace

export interface ToolManifest {
  id: string;
  name: string;
  category: CategoryId;
  icon: string;                       // 图标 key（全局图标表索引）
  description: string;
  keywords: string[];                 // 搜索同义词
  hotkey?: string;
  presentation: Presentation;         // 默认 'modal'；第二批大工具声明 'workspace'
  component: () => Promise<{ default: Component }>;   // 懒加载工厂
  background?: boolean;               // 后台服务型（剪贴板/番茄钟）
  settingsSchema?: SettingsField[];   // 工具级设置声明（框架渲染设置表单，工具零成本获得设置持久化）
  tags?: string[];
}

export interface SettingsField {      // 工具设置声明式 schema
  key: string; type: 'toggle' | 'text' | 'number' | 'select' | 'secret';
  label: string; default?: unknown; options?: { label: string; value: string }[];
}

export interface Settings {
  theme: 'light' | 'dark' | 'system';
  language: 'zh-CN' | 'en-US';
  globalHotkey: string;               // 默认 'Ctrl+Shift+Space'
  launchAtStartup: boolean;
  clipboard: { enabled: boolean; historyLimit: number; ignore: string[] };
  recentTools: string[];
  tools: Record<string, Record<string, unknown>>;  // 工具级设置，按工具 id 分区
}

// 第二批预留：连接型工具的统一会话模型
export interface ConnectionProfile {  // DB / SSH 通用
  id: string; name: string; kind: 'mysql' | 'postgres' | 'sqlite' | 'ssh';
  host?: string; port?: number; user?: string;
  secretRef?: string;                 // 可选公共 Vault 引用；为空时插件可保留手工凭据路径
  options?: Record<string, string>;
}

export interface ClipboardRecord {
  id: string; kind: 'text' | 'image' | 'file';
  content: string; preview: string; pinned: boolean; createdAt: number;
}
```

**持久化策略：**

| 数据 | 载体 | 理由 |
|------|------|------|
| Settings / 工具级设置 / favorites / recent | `tauri-plugin-store` | 小体积键值，读取即时 |
| 剪贴板历史 | `tauri-plugin-sql`（SQLite） | 查询/删除/上限管理 |
| 第二批：连接配置（host/user 等非敏感部分） | `tauri-plugin-store` | 与设置同级 |
| 第二批：密码/云 API Token/SSH 私钥 | `tauri-plugin-stronghold` | 加密落盘，明文只存内存 |

---

## 4. 工具注册表与扩展性设计

### 4.1 注册机制

```ts
// core/registry/toolRegistry.ts
const manifests = new Map<string, ToolManifest>();
export function registerTool(m: ToolManifest) { manifests.set(m.id, m); }
export function getTools(): ToolManifest[] { return [...manifests.values()]; }
export function getTool(id: string) { return manifests.get(id); }
```

- 工具目录内自注册（`tools/json-formatter/index.ts` 里 `registerTool(...)`）；
- **新增工具 = 新建目录 + 注册一行**，框架零改动——第一批的 8 个工具就是注册表扩展性的验收用例；
- 分类是数据不是枚举：侧栏分类由注册表按 `category` 聚合生成，新增分类只改类型联合 + 图标表。

### 4.2 Presentation 双载体（第一批做骨架，第二批受益）

- `modal`：轻量弹窗（第一批全部工具），沿用原型 002 的 ToolModal；
- `workspace`：全内容区工作台（第二批 HTTP/WS、DB 工具需要），`core/presentation/` 提供统一的挂载/卸载/生命周期接口，第一批实现 modal，workspace 留接口 + 最小骨架；
- 工具不关心载体，框架按 `manifest.presentation` 路由。

### 4.3 Rust 侧模块化（第二批预留的形态）

第一批的 Rust 只实现 `modules/settings|clipboard|color`，但**模块边界按第二批的形状划分**，每个复杂工具一个 module：

| 预留模块 | 第二批工具 | 技术形态 |
|---------|-----------|---------|
| `modules/http_ws` | HTTP/WS 调试 | reqwest + tokio-tungstenite（Rust 侧发请求，无 CORS） |
| `modules/db` | MySQL/PG/SQLite | sqlx（async，三方言统一）；连接池与会话存 `AppState` |
| `modules/hosts` | hosts 修改 | 读 `C:\Windows\System32\drivers\etc\hosts` + **按需提权**（见 §7） |
| `modules/dns` | 阿里/腾讯/CF | 统一 `Provider` trait + 三个 adapter（签名：阿里 HMAC-SHA1 / 腾讯 TC3-HMAC-SHA256 / CF Bearer） |
| `modules/ssh` | SSH 工具 | russh（纯 Rust async）；密钥管理走 secrets |
| `modules/secrets` | 以上共用 | stronghold 封装：存取 Token/密码/私钥 |

**Adapter 模式是第二批的骨架**：`dns::Provider`、`db::Connector`、`ssh::Client` 都是 trait + 实现，新增云厂商/数据库 = 新增 adapter，不动框架。

---

## 5. IPC 契约

> 前端一律经 `core/ipc/ipc.ts` 调用；`contracts.ts` 为契约唯一事实源，与 Rust serde 同步（CI 抽查）。

### 5.1 第一批（框架 + 文本工具实际使用）

| 命令 | 参数 | 返回 | 模块 |
|------|------|------|------|
| `settings_get` | `{ key? }` | `Settings` | settings |
| `settings_set` | `{ key, value }` | `()` | settings |
| `clipboard_list` | `{ limit?, pinnedOnly? }` | `ClipboardRecord[]` | clipboard |
| `clipboard_delete` / `clipboard_clear` / `clipboard_toggle_pin` | `{ id? }` 等 | `()` | clipboard |
| `color_pick_screen` | `{}` | `{ hex, rgb }` | color |
| `window_toggle` / `window_hide` | `{}` | `{ visible }` | framework |
| `open_external` | `{ url }` | `()` | opener 插件 |

### 5.2 第二批（预留，命令签名先行，实现随批次交付）

| 命令（预留） | 参数 | 返回 | 说明 |
|------|------|------|------|
| `http_request` | `{ method, url, headers, body, timeout }` | `HttpResponse` | Rust 侧 reqwest |
| `ws_connect` / `ws_send` / `ws_close` | `{ id, url }` / `{ id, data }` | `()` | 会话经 `sessions` 管理 |
| `db_connect` | `ConnectionProfile` | `{ sessionId }` | sqlx 连接池 |
| `db_query` | `{ sessionId, sql, params? }` | `{ columns, rows }` | 运行时 SQL |
| `hosts_read` / `hosts_write` | `{}` / `{ content }` | `string` / `()` | 写前检查权限，失败走提权流程 |
| `dns_list_zones` / `dns_list_records` / `dns_upsert_record` | `{ provider, credRef, … }` | `Zone[]` / `Record[]` | Provider trait 分发 |
| `ssh_connect` / `ssh_exec` / `ssh_disconnect` | `{ profile, command? }` | `{ sessionId }` / `{ stdout, stderr }` | russh |

> 第二批命令现在**只写进契约文档**，Rust 侧不建空壳——避免死代码；第一批框架只需保证"新增命令 = 注册一行"的装配方式（宏/统一函数）。

---

## 6. UI 组件树（第一批交付范围）

```
App.vue
├── Sidebar.vue           # 分类导航 + 计数徽标 + 主题切换 + 设置入口
├── TopBar.vue            # 标题 + 计数副标题 + SearchBox(Ctrl+K) + 视图切换 + 添加按钮
├── RecentStrip.vue       # 最近使用 Chips（≤6）
├── ToolGrid.vue / ToolList.vue   # 双视图（auto-fill 响应式）
│   └── ToolCard.vue      # 图标块 + 标题 + 描述 + 标签 + 收藏星
├── ToolModal.vue         # 弹窗载体：标题/描述/关闭 + <component :is> 懒加载
├── ToolWorkspace.vue     # 第二批预留：全内容区载体（第一批只实现空壳 + 路由）
├── SettingsModal.vue     # 外观/快捷键/开机自启/剪贴板策略/语言 + 工具级设置(由 settingsSchema 渲染)
└── Toast.vue
```

**第一批交互规格**（对齐原型 002）：卡片 hover 上浮 + 顶部渐变线；搜索即过滤 + 高亮；弹窗 180ms 上浮、Esc/遮罩关闭；工具卸载清理副作用；收藏/最近使用实时联动。

---

## 7. 安全设计

**第一批落地：**
- Capabilities 白名单（`capabilities/default.json`，最小权限，示例见技术选型文档）；
- 严格 CSP（零远程资源，`default-src 'self'`）；
- 禁用 `tauri-plugin-shell`；外链一律 `opener`。

**第二批关键决策（架构已预留，实现时落地）：**
- **网络请求走 Rust 侧**（reqwest / tokio-tungstenite），前端零 CORS、无 `http` 插件 scope 管理负担；
- **凭据加密**：公共 Vault 用系统 keyring 保存主密钥、AES-256-GCM 保存凭据本体；数据库、SSH、DNS 可存凭据引用并在 Rust 内解析，SSH/DNS 同时保留原手工输入兼容路径；
- **hosts 提权策略**：应用本体保持非提权运行；写 hosts 时检测权限，失败则通过 `std::process` 拉起 PowerShell `Start-Process -Verb RunAs` 的**最小化提权助手**（只执行 hosts 写入），UAC 按需弹出，不常驻管理员权限；
- DB/SSH 会话默认超时与空闲回收，防连接泄漏。

---

## 8. 设置项设计

| 分组 | 设置项 | 默认值 | 实现 |
|------|--------|--------|------|
| 外观 | 主题 / 语言 | 跟随系统 / zh-CN | `data-theme` / vue-i18n |
| 快捷键 | 全局唤起 | Ctrl+Shift+Space | global-shortcut |
| 通用 | 开机自启 | 关 | autostart |
| 剪贴板 | 启用 / 上限 / 忽略 | 开 / 200 / 空 | watcher + SQLite |
| **工具** | **各工具设置** | — | **框架渲染 `settingsSchema`，按 id 存 `settings.tools`** |

---

## 9. 工程约束：可读性与可扩展性（第一批验收红线）

**可读性：**
- 组件 < 300 行；逻辑抽纯函数（`tools/*/useXxx.ts`），UI 与逻辑分离——第一批 8 个工具全部遵循；
- 命名：文件 kebab-case、组件 PascalCase、Rust snake_case；语义化命名优先于注释；
- 契约集中：IPC 出入参只在 `contracts.ts` 出现一次；
- 代码评审清单：ESLint 9 + Prettier + `rustfmt` + `clippy -D warnings` 全绿才可合入。

**可扩展性（第二批验收标准 = 不破坏框架）：**
- 新增工具：`tools/<id>/` 目录 + `registerTool()` 一行；
- 新增 Rust 命令：`modules/` 下新模块 + 注册一行（装配处集中）；
- 新增云厂商/数据库/SSH 实现：实现 trait 的 adapter，零框架改动；
- 新增 presentation：`core/presentation/` 注册新载体，manifest 声明即用；
- 升级到第二批时，第一批代码**只增不改**（新增模块/工具/命令，不重构既有路径）。

---

## 10. 里程碑路线图

> 状态更新（2026-08-24）：M0–M3 已完成，项目进入 M4 发布准备。凭据方案已由 Stronghold 调整为系统 keyring + AES-256-GCM，数据库、SSH、DNS 均已支持 Vault 引用。

| 阶段 | 状态 | 周期 | 交付物 | 验收标准 |
|------|------|------|--------|---------|
| **M0 脚手架** | 已完成 | 1–2 天 | create-tauri-app(vue-ts) + Tailwind 4 接 DESIGN.md tokens + ESLint/Prettier/rustfmt + CI | `pnpm tauri dev` 出方向二主界面（空数据版），CI 绿 |
| **M1 框架 + 第一批** | 已完成 | 5–8 天 | 整体 UI（§6 全部组件）+ 后端框架（§1 框架层 + 托盘/快捷键/单实例）+ 文本工具 | 浏览、搜索、收藏、换肤、设置持久化闭环；质量门槛全绿 |
| **M2 第二批 I** | 已完成 | 5–7 天 | HTTP/WS 调试、SQLite 数据库工具、hosts 修改 + 轻量工具 | workspace 载体上线；三工具可用；hosts 提权流程走通 |
| **M3 第二批 II** | 已完成 | 7–10 天 | 多驱动数据库工作台、DNS 管理、SSH 远程管理、公共 Vault | 数据库、SSH、阿里/腾讯/Cloudflare DNS 与可选 Vault 引用完成 |
| **M4 打磨发布** | 进行中 | 持续 | 扩展工具、i18n 英文、自动更新、代码签名、发布验证 | CI 已配置 Windows/macOS 产物构建；其余发布能力待完成 |

**第一批工具清单（8 个，全为文本类）：** JSON 格式化、时间戳转换、Base64 编解码、URL 编解码、字符统计、文本对比、Markdown 预览、正则测试。

---

## 11. 开放决策（进入开发前确认）

1. ~~项目名~~ → 已定：**patchyBox**，identifier 默认 `com.patchy23.patchybox`；
2. 前端框架默认 **Vue 3**（M0 前可改 React，影响面见技术选型 §2）；
3. 关窗行为：默认最小化到托盘；
4. 剪贴板隐私：默认仅忽略列表；
5. ~~第二批凭据方案~~ → 已定：**系统 keyring 保存主密钥 + AES-256-GCM 加密凭据文件**，Stronghold 已弃用；
6. hosts 提权：默认**按需提权助手**（UAC 弹出最小授权），不整体管理员运行。
