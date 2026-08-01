# 详细设计 · ToolKit 桌面工具箱

> 对应已确认的设计方向：**方向二 · 明净浅色（内容优先）**，视觉规范见仓库根目录 `DESIGN.md`。
> 技术栈见 `01-tech-stack.md`：Tauri 2.11 + Vue 3.5 + TS + Tailwind 4 + Pinia。

---

## 1. 总体架构

```
┌────────────────────────────── 前端 (src/) ──────────────────────────────┐
│  features/  Sidebar · TopBar · ToolGrid · ToolList · ToolModal · Toast  │
│        ▲ 使用                                                           │
│  core/    toolRegistry(注册表) · search(模糊搜索) · ipc(类型安全调用)      │
│        ▲ 读写                                                           │
│  stores/ tools · settings · favorites · clipboard · ui   (Pinia)        │
│        ▲ 懒加载                                                        │
│  tools/  json-formatter · ts-converter · base64 · …  (内置工具实现)      │
└──────────────────────────────────┬──────────────────────────────────────┘
                                   │ invoke() / listen()   (类型安全, core/ipc 封装)
┌────────────────────────────── Rust (src-tauri/) ───────────────────────┐
│  commands/  settings · clipboard · color · disk · window · …            │
│  plugins    clipboard-manager · global-shortcut · store · sql · …       │
│  state/     AppState (连接池、运行时配置)                                │
│  系统能力   屏幕取色 · 磁盘扫描 · 剪贴板轮询 · 托盘 · 全局快捷键           │
└──────────────────────────────────────────────────────────────────────────┘
```

**核心数据流（示例：用户打开「剪贴板历史」）：**

```
ToolGrid 点击卡片
  → ToolModal 挂载, dynamic import 加载工具组件（首次）
  → 工具组件调用 core/ipc.getClipboardHistory()
  → Tauri invoke → Rust command: clipboard::list()
  → rusqlite 查询 → JSON 序列化返回
  → Pinia clipboard store 缓存 + 组件渲染列表
```

---

## 2. 目录结构

```
toolbox-ui/
├── DESIGN.md                  # 设计 tokens 规范（单一事实源）
├── docs/                      # 本文档 + 技术选型
├── index.html
├── package.json
├── vite.config.ts             # @vitejs/plugin-vue + auto-import + 路径别名
├── src/
│   ├── main.ts
│   ├── App.vue                # 布局骨架：Sidebar + TopBar + 内容区
│   ├── assets/styles/main.css # Tailwind 4 @theme（对接 DESIGN.md tokens）
│   ├── core/
│   │   ├── ipc/ipc.ts         # invoke/listen 的 Promise 封装 + 错误归一化
│   │   ├── ipc/contracts.ts   # IPC 出入参 TS 类型（与 Rust 侧契约同步）
│   │   ├── registry/toolRegistry.ts   # 工具注册表（核心，见 §4）
│   │   ├── registry/types.ts          # ToolManifest 等接口
│   │   ├── search/fuzzy.ts            # fuse.js 封装 + 高亮
│   │   └── hotkeys/hotkeys.ts         # 应用内快捷键绑定
│   ├── features/
│   │   ├── sidebar/Sidebar.vue        # 分类导航 + 收藏 + 底部设置入口
│   │   ├── topbar/TopBar.vue          # 标题 + 搜索框 + 视图切换 + 添加按钮
│   │   ├── grid/ToolGrid.vue          # 卡片网格视图
│   │   ├── grid/ToolList.vue          # 列表视图
│   │   ├── grid/RecentStrip.vue       # 最近使用快捷条
│   │   ├── modal/ToolModal.vue        # 工具运行容器（懒加载 + 生命周期）
│   │   ├── settings/SettingsModal.vue # 设置（主题/快捷键/剪贴板/语言）
│   │   └── ui/                        # Button · Card · Chip · Modal · Toast · Toggle
│   ├── tools/                          # 每个工具一个目录，独立可测
│   │   ├── json-formatter/{index.vue, useFormat.ts}
│   │   ├── ts-converter/  base64/  password-gen/  char-stat/ …
│   ├── stores/               # pinia: tools.ts settings.ts favorites.ts clipboard.ts ui.ts
│   └── locales/zh-CN.ts      # vue-i18n
├── src-tauri/
│   ├── Cargo.toml            # tauri 2.11 + 插件依赖
│   ├── tauri.conf.json       # 窗口配置/打包/标识符 com.yourname.toolkit
│   ├── capabilities/default.json      # 权限白名单（见 §7）
│   ├── icons/
│   ├── src/
│   │   ├── main.rs           # 入口：plugins 注册 + 命令注册 + tray + 单实例
│   │   ├── lib.rs            # Builder 装配（供测试复用）
│   │   ├── state.rs          # AppState：SqlitePool、剪贴板轮询句柄
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── settings.rs   # get/set 设置
│   │   │   ├── clipboard.rs  # 历史列表/删除/清空/置顶
│   │   │   ├── color.rs      # 屏幕取色
│   │   │   ├── disk.rs       # 磁盘扫描（异步 + 进度事件）
│   │   │   └── window.rs     # 主窗显隐切换
│   │   └── services/
│   │       ├── clipboard_watcher.rs   # 500ms 轮询 + hash 去重 + 落库
│   │       └── color_picker.rs        # GetDC/GetPixel 取色
│   └── tests/                # Rust 命令单测
└── e2e/                      # Playwright（可选，M4）
```

---

## 3. 数据模型（TS 接口，与 Rust serde 结构一一对应）

```ts
// core/registry/types.ts
export type CategoryId = 'dev' | 'text' | 'image' | 'net' | 'sys';

export interface ToolManifest {
  id: string;                     // 唯一 id，如 'json-formatter'
  name: string;                   // 显示名
  category: CategoryId;
  icon: string;                   // 图标 key（全局图标表索引）
  description: string;
  keywords: string[];             // 搜索用同义词（如 ['格式化','pretty','json']）
  hotkey?: string;                // 应用内快捷键（如 'Ctrl+1'）
  component: () => Promise<{ default: Component }>;  // 懒加载工厂
  background?: boolean;           // true = 后台服务型（剪贴板/番茄钟），无 UI 弹窗
  tags?: string[];                // 展示用标签（'热门' 等）
}

export interface Settings {
  theme: 'light' | 'dark' | 'system';
  language: 'zh-CN' | 'en-US';
  globalHotkey: string;           // 全局唤起组合键，默认 'Ctrl+Shift+Space'
  launchAtStartup: boolean;
  clipboard: {
    enabled: boolean;
    historyLimit: number;         // 默认 200
    ignore: string[];             // 忽略的窗口/内容模式
  };
  recentTools: string[];          // 最近使用（最多 6）
}

export interface ClipboardRecord {
  id: string;                     // uuid
  kind: 'text' | 'image' | 'file';
  content: string;                // 文本内容；图片存缩略图路径
  preview: string;                // 列表预览（截断 120 字）
  pinned: boolean;
  createdAt: number;              // epoch ms
}
```

**持久化策略：**

| 数据 | 载体 | 理由 |
|------|------|------|
| Settings / favorites / recent | `tauri-plugin-store`（`settings.json`） | 小体积键值，读取即时 |
| 剪贴板历史 | `tauri-plugin-sql`（SQLite `clipboard.db`） | 上限 200+ 条、需要查询与删除 |
| 工具集合（内置） | 代码内静态注册 | 无运行时变更 |
| 未来：用户自定义工具 | 独立 `tools.user.json` | M4 插件化预留 |

---

## 4. 工具注册表（核心架构）

工具箱的本质是"工具的可插拔集合"。注册表统一收口：

```ts
// core/registry/toolRegistry.ts
import type { ToolManifest } from './types';

const manifests = new Map<string, ToolManifest>();

export function registerTool(m: ToolManifest) { manifests.set(m.id, m); }
export function getTools(): ToolManifest[] { return [...manifests.values()]; }
export function getTool(id: string) { return manifests.get(id); }

// 分类聚合（侧栏计数复用）
export function toolsByCategory(cat: CategoryId | 'all' | 'fav', favIds: Set<string>) { … }
```

- **内置工具在 `tools/` 各目录内自注册**（`registerTool(...)`），新增工具 = 新建目录 + 注册一行，不动框架代码；
- **懒加载**：`ToolModal` 首次打开时才 `component()` 动态 import，首屏只加载框架与图标；
- **搜索**：fuse.js 索引 `name + keywords + description`，权重 name 3 > keywords 2 > desc 1；
- **热键**：`hotkey` 字段注册应用内快捷键（`hotkeys.ts` 统一管理，避免多工具冲突）；
- **后台工具**：`background: true` 的工具不渲染弹窗，只向 Rust 注册常驻服务（如剪贴板监听），在设置页显示开关与状态。

### 内置工具清单与排期

| 工具 | 分类 | 实现形态 | 里程碑 |
|------|------|---------|--------|
| JSON 格式化 | dev | 前端（演示已实现 ✅） | M2 |
| 时间戳转换 | dev | 前端 | M2 |
| Base64 编解码 | dev | 前端 | M2 |
| 随机密码 | sys | 前端 | M2 |
| 字符统计 | text | 前端 | M2 |
| 文本对比 | text | 前端（左右分栏） | M2 |
| Markdown 预览 | text | 前端（marked.js） | M2 |
| 单位换算 | sys | 前端 | M2 |
| URL 编解码 | dev | 前端 | M2 |
| 正则测试 | dev | 前端（实时高亮） | M3 |
| 哈希计算 | dev | Rust（md-5/sha2 crate）+ 前端 | M3 |
| 颜色选择器 | image | 前端 + 自研取色命令 | M3 |
| 屏幕取色 | image | Rust（GetDC/GetPixel） | M3 |
| 剪贴板历史 | sys | Rust 轮询 + SQLite + 前端 | M3 |
| 番茄钟 | sys | Rust notification + 前端 | M3 |
| 汇率换算 | net | 前端 + HTTP 插件（离线缓存） | M3 |
| 二维码生成 | image | 前端（qrcode.js） | M3 |
| 图片压缩 | image | Rust（image crate）+ dialog/fs | M4 |
| 批量重命名 | sys | Rust（fs 遍历 + 预览） | M4 |
| 磁盘分析 | sys | Rust（walkdir 异步 + 进度事件） | M4 |
| 快捷翻译 | text | HTTP 插件 + 多引擎适配 | M4 |

---

## 5. IPC 契约

> 前端一律经 `core/ipc/ipc.ts` 调用（封装错误归一化 + loading 态），**禁止**在组件里裸写 `invoke`。
> `contracts.ts` 的类型是契约唯一事实源，Rust 侧 `serde` 结构体必须与之同步（CI 里加类型一致性抽查）。

| 命令 `toolkit://` | 参数 | 返回 | 说明 |
|-------------------|------|------|------|
| `settings_get` | `{ key?: string }` | `Settings` | 全量或单键 |
| `settings_set` | `{ key, value }` | `()` | 写 store 文件 |
| `clipboard_list` | `{ limit?, pinnedOnly? }` | `ClipboardRecord[]` | 分页查询 |
| `clipboard_delete` | `{ id }` | `()` | |
| `clipboard_clear` | `{}` | `()` | |
| `clipboard_toggle_pin` | `{ id, pinned }` | `()` | |
| `color_pick_screen` | `{}` | `{ hex, rgb }` | 全屏取色（自研） |
| `disk_scan` | `{ path? }` | `DiskReport` | 事件 `disk://progress` 推送进度 |
| `hash_text` | `{ text, algos[] }` | `Record<algo, string>` | md5/sha1/sha256/sha512 |
| `image_compress` | `{ srcPath, quality, maxWidth? }` | `{ outPath, before, after }` | |
| `open_external` | `{ url }` | `()` | opener 插件 |
| `window_toggle` | `{}` | `{ visible }` | 全局快捷键呼出/隐藏 |
| `window_hide` | `{}` | `()` | 失焦隐藏（可选） |

**事件（前端 `listen`）：**
- `clipboard://changed` — 剪贴板新增记录（前台实时刷新）
- `disk://progress` — 扫描进度 `{ current, total, path }`
- `tray://menu` — 托盘菜单动作（显示主窗/退出）

---

## 6. UI 组件树（从方向二原型映射）

```
App.vue
├── Sidebar.vue
│   ├── NavItem × n          # 分类 + 计数徽标（全部/开发/文本/图片/网络/系统/收藏）
│   ├── ThemeToggle          # 浅色/深色/跟随系统
│   └── SettingsEntry
├── TopBar.vue
│   ├── 标题 + 计数副标题
│   ├── SearchBox             # 聚焦高亮 + Ctrl+K
│   ├── ViewToggle            # 网格/列表
│   └── AddButton（+）
├── RecentStrip.vue           # 最近使用 Chips（最多 6，点击即开）
├── ToolGrid.vue / ToolList.vue   # 双视图（响应式 auto-fill）
│   └── ToolCard.vue          # 图标块 + 标题 + 描述 + 标签 + 收藏星
├── ToolModal.vue             # 弹窗容器：标题/描述/关闭 + <component :is> 懒加载工具
├── SettingsModal.vue         # 主题 / 全局快捷键 / 开机自启 / 剪贴板策略 / 语言
└── Toast.vue                 # 底部居中提示（收藏/复制等反馈）
```

**关键交互规格（对齐原型 002）：**
- 卡片 hover：上浮 3px + 顶部 3px 橙色渐变线；点击开弹窗；收藏星 stopPropagation；
- 搜索：输入即过滤（fuse），高亮命中片段；`Ctrl+K` 聚焦；
- 弹窗：180ms 上浮淡入、`Esc`/遮罩点击关闭；工具组件卸载时清理其副作用（定时器/监听）；
- 空状态：搜索无结果时展示"换个关键词"引导（原型已有）；
- 最近使用：工具打开后写入 `settings.recentTools`（去重、上限 6）。

---

## 7. 安全设计

**Capabilities 白名单**（最小权限，`capabilities/default.json`）：

```json
{
  "identifier": "default",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "store:default",
    "clipboard-manager:allow-read-text",
    "clipboard-manager:allow-write-text",
    "global-shortcut:allow-register",
    "global-shortcut:allow-unregister",
    "dialog:allow-open",
    "opener:default",
    "sql:allow-load",
    "sql:allow-execute",
    "sql:allow-select"
  ]
}
```

- **CSP**：`tauri.conf.json` 中设置 `"csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: asset:"`——本项目零远程资源，可做到最严；
- **禁用** `shell` 插件与 `http` 插件的任意 URL 访问（翻译/汇率仅白名单域名）；
- 前端不直接拿文件系统路径：图片压缩等经 dialog 选择 + Rust 处理，前端只见结果；
- 剪贴板数据含敏感信息：默认只保留纯文本、提供"清空全部"与单条删除、设置页说明存储位置（`AppData` 内）。

---

## 8. 设置项设计

| 分组 | 设置项 | 默认值 | 实现 |
|------|--------|--------|------|
| 外观 | 主题 | 跟随系统 | `data-theme` + `window.matchMedia` |
| 外观 | 语言 | zh-CN | vue-i18n |
| 快捷键 | 全局唤起 | `Ctrl+Shift+Space` | global-shortcut（失败提示改键） |
| 通用 | 开机自启 | 关 | autostart 插件 |
| 剪贴板 | 启用历史 | 开 | clipboard watcher |
| 剪贴板 | 上限条数 | 200 | SQLite 清理策略 |
| 剪贴板 | 忽略应用/内容 | 空 | 前缀匹配 |
| 关于 | 版本/更新 | — | updater（M4） |

---

## 9. 测试策略

| 层 | 工具 | 覆盖点 |
|----|------|--------|
| 前端单元 | Vitest + @vue/test-utils | toolRegistry、fuzzy 搜索、stores（favorites/settings）、每个工具的核心逻辑（json 格式化/时间戳/base64 已有纯函数 ✅） |
| Rust 单元 | `cargo test` | 剪贴板去重、SQLite 增删查、取色/哈希/压缩命令的纯逻辑 |
| 契约 | CI 脚本 | contracts.ts ↔ Rust serde 字段名抽查 |
| 手动验收 | 清单 | 双主题 × 双视图 × 21 工具 × 全局快捷键 × 托盘 × 单实例 |

---

## 10. 里程碑路线图

| 阶段 | 周期 | 交付物 |
|------|------|--------|
| **M0 脚手架** | 1–2 天 | create-tauri-app(vue-ts) + Tailwind 4 接入 DESIGN.md tokens + ESLint/Prettier/rustfmt + CI（lint/test/build artifact） |
| **M1 应用框架** | 3–5 天 | 注册表 + 搜索 + 双视图 + 收藏 + 最近使用 + 主题 + 设置弹窗 + 托盘 + 全局唤起 + 单实例 |
| **M2 首批工具** | 3–5 天 | 8 个纯前端工具（清单 §4 的 M2 行） |
| **M3 系统能力** | 5–7 天 | 剪贴板历史、屏幕取色、颜色选择器、哈希、正则、番茄钟、汇率、二维码 |
| **M4 打磨发布** | 持续 | 图片压缩、批量重命名、磁盘分析、翻译、i18n 英文、自动更新、代码签名、插件化预留 |

**M0 验收标准**：`pnpm tauri dev` 一跑即出方向二的主界面（空数据版），CI 绿。
**M1 验收标准**：无任何工具也能完整体验"浏览-搜索-收藏-换肤"闭环；全局快捷键呼出窗口。

---

## 11. 开放决策（进入开发前需确认）

1. **前端框架**：默认 Vue 3，若团队 React 更熟请在 M0 前提出（影响面见技术选型 §2）；
2. **应用标识符**：`tauri.conf.json` 的 `identifier`（如 `com.yourname.toolkit`）与窗口名、产品名；
3. **托盘行为**：关窗 = 最小化到托盘（推荐）还是直接退出；
4. **剪贴板隐私**：是否需要"密码类内容不记录"的启发式过滤（默认只做忽略列表）。
