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

- **工具注册表**（`src/core/registry/`）：工具目录自注册，新增工具 = 建目录 + 注册一行，框架零改动
- **presentation 双载体**：`modal`（第一批全用，原型 002 的弹窗）/ `workspace`（第二批大工具，第一批只留接口+空壳）
- **Rust 模块化**：`src-tauri/modules/` 按第二批形状划分（settings/clipboard/color 第一批；http_ws/db/hosts/dns/ssh/secrets 第二批），Adapter 模式（Provider trait）是第二批骨架
- **IPC 契约唯一事实源**：`src/core/ipc/contracts.ts`（与 Rust serde 同步；命令清单见 02-architecture §5）
- **工具级设置**：manifest 声明 `settingsSchema`，框架自动渲染设置表单并存 `settings.tools[id]`

## 文档地图

| 文件                                               | 内容                                                             |
| -------------------------------------------------- | ---------------------------------------------------------------- |
| `DESIGN.md`                                        | 设计 tokens：28 色 / 18 组件变体，已过 `designmd lint`（0 错误） |
| `docs/01-tech-stack.md`                            | 技术选型 + 第二批技术预备评估                                    |
| `docs/02-architecture.md`                          | 两批次详细设计：架构 / 注册表 / IPC / 安全 / 路线图 M0–M4        |
| `sketches/002-clean-light/`                        | 已采纳方向的**可交互原型**，开发验收视觉参照                     |
| `sketches/001-command-dark/` `003-glass-launcher/` | 未采纳方向，留档勿删                                             |

## 关键设计约束（来自 DESIGN.md，务必遵守）

- 交互驱动色 `tertiary #F0562C`，但**小号白字按钮必须用 `tertiary-strong #C2410C`**（WCAG AA）
- 选中态文字（nav/chip active）用 tertiary-strong；HOT 标签用 `success-strong #067647`
- 深色模式走 `-dark` token 变体，不新造色值；卡片网格 `auto-fill minmax(228px,1fr)`

## 工程约束（第一批验收红线）

**可读性**：组件 < 300 行；逻辑抽纯函数（`tools/<id>/useXxx.ts`）；IPC 出入参只在 contracts.ts 出现一次；ESLint 9 + Prettier + rustfmt + `clippy -D warnings` 全绿才合入。
**可扩展性**：新增工具 = 注册一行；新增 Rust 命令 = 模块 + 装配处注册一行；新增厂商/数据库 = adapter；升级第二批时第一批代码只增不改。

## 开放问题（默认值已定，开工前可与用户确认）

1. 前端框架：默认 **Vue 3**（React 需在 M0 前提出，改动面见技术选型 §2）
2. 应用 identifier：默认 `com.patchy23.patchybox`（tauri.conf.json）
3. 关窗行为：默认**最小化到托盘**
4. 剪贴板隐私：默认仅忽略列表，不做启发式过滤
5. 第二批凭据：默认 **stronghold**（备选 Windows Credential Manager / keyring）
6. hosts 提权：默认**按需提权助手**（UAC 最小授权），应用本体不常驻管理员

## 下一步：M2 第二批 I（新会话任务）

M0/M1 已完成（脚手架 + 框架 + 8 个文本工具，验收全绿）。M2 按 docs/02-architecture.md §10 推进：

1. **workspace 载体上线**：`core/presentation/` 补 workspace 挂载（第一批已留接口 + ToolWorkspace 空壳），HTTP/WS、DB 工具用全内容区工作台
2. **HTTP/WS 调试**：Rust `modules/http_ws`（reqwest + tokio-tungstenite，第二批依赖此时引入）
3. **SQLite 数据库工具**：`modules/db`（sqlx 统一三方言，先 SQLite）
4. **hosts 修改**：`modules/hosts` + 按需提权助手（UAC 最小授权）+ 修改前备份
5. **轻量工具补齐**：随机密码 / 哈希计算 / 颜色选择器（color_pick_screen 命令已就绪）/ 二维码
6. **剪贴板历史工具**：clipboard 模块 + 轮询服务 + SQLite 历史已在 M1 就绪，补 UI 工具即可

### M1 环境备忘补充

- 全局快捷键 Ctrl+Shift+Space 在本机被占用时降级告警（`[shortcut] 注册失败`），设置页改键功能 M2 接入（Rust 侧注册读取 settings.globalHotkey）
- 剪贴板历史 db：`%APPDATA%/com.patchy23.patchybox/clipboard.db`（rusqlite bundled）；历史上限清理未实现（M2 接 settings.clipboard.historyLimit）

### M0 环境备忘（Windows）

- **pnpm 命令**：系统 corepack shim 损坏（`G:\d\base\nodejs\pnpm` 报 MODULE_NOT_FOUND），需先 `export PATH="/c/Users/patchy/AppData/Roaming/npm:$PATH"`（npm 全局 pnpm 11.18.0）再执行 pnpm
- **pnpm 11 配置**：settings 在 `pnpm-workspace.yaml`（`allowBuilds: { esbuild: true }`），package.json 的 `pnpm` 字段已废弃不再读取

## 约定

- 文档与注释中文；代码标识符英文；提交信息中文（如 `feat: 添加工具注册表`）
- 所有命令在 git-bash 执行；Windows 环境
- 改 DESIGN.md 后跑 `npx -y -p @google/design.md designmd lint DESIGN.md` 校验
