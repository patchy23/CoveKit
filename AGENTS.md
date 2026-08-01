# patchyBox · 项目简报（AGENTS.md）

> 本文件在会话打开 `G:\workspace\patchyBox` 时自动注入，先读它再动手。
> 设计阶段已完成（2026-08-02），当前状态：**M0 脚手架未开始**。

## 项目是什么

基于 **Tauri 2** 的桌面工具箱（patchyBox），集成常用开发与效率工具，按类别分组、可扩展（预留插件化）。
UI 方向已定稿：**明净浅色 · 内容优先**（macOS 式侧栏 + 大卡片网格），原型见 `sketches/002-clean-light/`。

## 已定技术决策（详见 docs/01-tech-stack.md）

- 容器：Tauri 2.11（Rust）+ 官方插件：clipboard-manager / global-shortcut / store / sql / single-instance / autostart / notification / window-state / updater / dialog / fs / opener / positioner
- 前端：**Vue 3.5 + TypeScript + Vite + Tailwind CSS 4 + Pinia** + fuse.js（搜索）+ vue-i18n（zh-CN 默认）
- 不引入重型组件库（Element/Naive 等），UI 自研，tokens 以根目录 `DESIGN.md` 为**单一事实源**
- 明确禁用 `tauri-plugin-shell`（安全）
- 架构核心：**工具注册表**（`src/core/registry/`）——新增工具 = 新建 `src/tools/<id>/` 目录 + 注册一行，工具组件懒加载
- IPC 契约唯一事实源：`src/core/ipc/contracts.ts`（与 Rust serde 结构体同步；命令清单见 docs/02-architecture.md §5）

## 文档地图

| 文件 | 内容 |
|------|------|
| `DESIGN.md` | 设计 tokens：28 色 / 6 字号 / 18 组件变体，已通过 `designmd lint`（0 错误） |
| `docs/01-tech-stack.md` | 技术选型分析与理由 |
| `docs/02-architecture.md` | 架构分层 / 目录结构 / 数据模型 / IPC 契约表 / 安全 / 路线图 M0–M4 |
| `sketches/002-clean-light/` | 已采纳方向的**可交互原型**，开发验收的视觉参照 |
| `sketches/001-command-dark/` `003-glass-launcher/` | 未采纳方向，留档勿删 |

## 关键设计约束（来自 DESIGN.md，务必遵守）

- 交互驱动色 `tertiary #F0562C`，但**小号白字按钮必须用 `tertiary-strong #C2410C`**（WCAG AA）
- 选中态文字（nav/chip active）用 tertiary-strong；HOT 标签用 `success-strong #067647`
- 深色模式走 `-dark` token 变体，不新造色值
- 卡片网格 `auto-fill minmax(228px,1fr)`；间距基线 4px

## 开放问题（用户未拍板，以下为默认值，开工前可与用户确认）

1. 前端框架：默认 **Vue 3**（若团队用 React 需在 M0 前提出，改动面见技术选型 §2）
2. 应用 identifier：默认 `com.patchy23.patchybox`（tauri.conf.json）
3. 关窗行为：默认**最小化到托盘**
4. 剪贴板隐私：默认仅忽略列表，不做启发式过滤
5. 托盘菜单：默认含「显示主窗 / 退出」

## 下一步：M0 脚手架（新会话任务）

1. `pnpm create tauri-app`（vue-ts 模板）脚手架装入本目录（或手工搭建：Vite + Vue3 + TS）
2. Tailwind CSS 4 接入 DESIGN.md tokens（`@theme` + `@custom-variant dark` 绑定 `[data-theme=dark]`），见 docs/01-tech-stack.md §3 示例
3. 工程化：ESLint 9 + Prettier、rustfmt + clippy（`-D warnings`）、路径别名 `@/`
4. CI：GitHub Actions（lint → test → `tauri build` artifact）
5. 验收标准：`pnpm tauri dev` 跑出方向二主界面（空数据版：侧栏分类 + 顶栏搜索 + 卡片网格 + 深浅主题切换）

## 约定

- 文档与注释使用中文；代码标识符英文
- 提交信息用中文描述（如 `feat: 添加工具注册表`）
- 所有命令在 git-bash 中执行；Windows 环境
- 原型目录与 docs 是设计期产物，后续改动需同步 DESIGN.md（跑 `npx -y -p @google/design.md designmd lint DESIGN.md` 校验）
