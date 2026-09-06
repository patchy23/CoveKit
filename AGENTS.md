# patchyBox · 项目简报（AGENTS.md）

> 本文件在会话打开 `G:\workspace\patchyBox` 时自动注入，先读它再动手。
> 本文只放**常青规则**；进度/历史看 git log 与 CHANGELOG.md，设计细节看 docs/。

## 项目是什么

基于 **Tauri 2** 的桌面工具箱 **patchyBox**（Windows+macOS）。UI：**明净浅色 · 内容优先**（macOS 式侧栏 + 大卡片网格），原型见 `sketches/002-clean-light/`。现有工具：格式转换、接口调试、数据库、Hosts 编辑、DNS 解析、SSH 远程管理、文字转语音、组件实验室（公共组件视觉验收页）。

## 已定技术决策（详见 docs/01-tech-stack.md）

- 容器：Tauri 2.11（Rust）+ 官方插件（clipboard-manager / global-shortcut / store / single-instance / autostart / notification / window-state / updater / dialog / fs / opener）
- 前端：**Vue 3.5 + TypeScript + Vite + Tailwind CSS 4 + Pinia** + fuse.js + vue-i18n（zh-CN 默认）
- 公共 UI：**shadcn-vue 源码模式 + Reka UI 无样式原语**，业务仅从 `@/core/ui` 使用 `Ui*`；tokens 以根目录 `DESIGN.md` 为单一事实源
- 明确禁用 `tauri-plugin-shell`；**禁单平台绑定的技术选型**
- 凭据：系统 keyring 主密钥 + AES-256-GCM 凭证文件（`framework/vault`），插件可选引用 + 手工输入双路径

## 架构核心（详见 docs/02-architecture.md）

- **插件模式（前后端同构）**：前端 `src/plugins/<id>/`（manifest 自注册 + contracts.ts/ipc.ts + 组件，插件间禁止互相 import）；公共能力走 `src/core/ui/` 与 `src/core/ipc/`；Rust 侧 `src-tauri/src/plugins/<id>/` 一律目录结构（mod.rs 门面 + models.rs + 能力子模块/子目录）
- **框架与插件分离**：`src-tauri/src/framework/`（设置/快捷键/窗口/命令入库/数据管理/vault）是基建不属于插件；**无实际前端引用的插件必须删除**
- **IPC 接口入库**（`framework/ipc_registry.rs`）：插件 register() 登记命令（owner + 名称 + 中文说明），启动校验全局唯一（重复即 panic）；路由按注册表 owner 精确匹配（`plugins/mod.rs` 一个分支/插件），`validate_routing()` 启动 fail-fast 校验死命令；**不再手写前缀清单**
- **数据库管理规则**（`framework/store.rs`）：插件数据文件统一 `app_data_dir/<plugin>.db`；表结构走 `PRAGMA user_version` 顺序迁移（只追加；duplicate column 幂等跳过）；统一骨架 `PluginDb`（连接生命周期 + 锁 + 迁移）
- **工具注册表**（`src/core/registry/`）：新增工具 = 建目录 + `plugins/index.ts` 一行，框架零改动
- **载体**：全部工具以多页签工作区子页面打开（`src/features/workspace/ToolWorkspace.vue`，页签支持 Ctrl+W 关闭、Ctrl(Shift)+Tab 循环、溢出三点收纳）
- **工具级设置**：manifest 声明 `settingsSchema`，框架自动渲染设置表单并存 `settings.tools[id]`

## 文档地图

| 文件 | 内容 |
| ---- | ---- |
| `DESIGN.md` | 设计 tokens：28 色 / 18 组件变体（改后跑 `npx -y -p @google/design.md designmd lint DESIGN.md`） |
| `docs/01-tech-stack.md` | 技术选型 |
| `docs/02-architecture.md` | 架构 / 注册表 / IPC / 安全 |
| `docs/03-plugin-development.md` | **插件开发规则：目录与规模红线 / IPC 入库 / 数据库管理 / 质量门槛 / Check-list** |
| `docs/05-rust-code-standard.md` | **Rust 代码规范 v1.1：panic/clone/生命周期/异步/脱敏/依赖评审；提交前跑 `scripts/check_rust_rules.py`（棘轮只减不增）** |
| `docs/06-release.md` | 发布、自动更新与代码签名 |
| `docs/07-product-requirements.md` | 产品需求文档 |

## 关键设计约束（来自 DESIGN.md，务必遵守）

- 交互驱动色 `tertiary #F0562C`，小号白字按钮用 `tertiary-strong #C2410C`（WCAG AA）；选中态文字 tertiary-strong；HOT 标签 `success-strong #067647`
- 深色模式走 `-dark` token 变体，不新造色值；卡片网格 `auto-fill minmax(228px,1fr)`
- **暗色覆盖写 `main.css` 全局 unlayered 区**：组件 scoped 样式里 `:global()+:deep()` 混写会被编译静默丢弃

## 工程约束（合入红线）

- **可读性**：组件 < 300 行；逻辑抽纯函数（`useXxx.ts` 必须带单测）；**Rust 单文件 >400 行或插件能力文件 >8 个必须下沉能力域子目录；前端插件 >15 个文件必须按特性域分目录**（细则 docs/03 §1）；IPC 出入参只在 contracts.ts 出现一次
- **字体规范**：字号只用语义 token（text-h1/text-brand/text-card-title/text-h2/text-body/text-body-sm/text-caption/text-label-caps/text-display），禁 arbitrary `text-[*px]`；字体族 `--font-sans`/`--font-mono`（font-mono 只用于明文数据/代码内容，密码框不加）
- **公共组件**：避免各插件自绘 UI 差异——密码框统一 `UiInput type="password"`（自带眼睛）、下拉 `UiSelect`、可搜索下拉 `UiCombobox`、列表行 `UiListRow`、右键菜单 `ContextMenu`、删除确认 `ConfirmDialog`、页签溢出 `UiTabsOverflow`；高度基线 36px
- **验证门禁**：`pnpm lint --max-warnings 0` + `pnpm format` + `pnpm test` + `pnpm build` + Rust `clippy -D warnings`（`--no-default-features`）+ `cargo test` + `check_docs.py` + `check_rust_rules.py` 全绿才合入
- **dev 冷启动冒烟**：改 `tauri.conf.json`/`lib.rs` 插件注册/`Cargo.toml`/能力权限时，提交前必须 `pnpm tauri dev` 冷启动确认无 panic（教训：updater 配置缺失曾致 dev 启动崩两周无人发现）
- **所有用户操作必须有可见反馈**（toast/错误行），禁止静默 catch
- **Tauri 已知坑**：`dragDropEnabled`（默认开）会吞掉应用内 HTML5 拖拽——内部拖拽用 pointer 事件自实现（参照 `src/plugins/ssh/useServerGroups.ts` 的 `useGroupDrag`）

## 环境备忘（Windows）

- **pnpm**：系统 corepack shim 损坏，先 `export PATH="/c/Users/patchy/AppData/Roaming/npm:$PATH"`（npm 全局 pnpm）再执行 pnpm；pnpm 11 配置在 `pnpm-workspace.yaml`（package.json 的 pnpm 字段已废弃）
- 所有命令在 git-bash 执行；Windows 环境
- cmdkey 输出为 GBK 且含 NUL，需 `iconv -f GBK -t UTF-8` 后 grep

## 约定

- 文档与注释中文；代码标识符英文
- **提交**：Conventional Commits `类型(scope): 中文描述`；标题一行总概括，body 按代码增删改分条、内容具体；类型按实质（搬移 refactor/隐患 fix/新机制 feat/测试 test）、scope 按真实改动模块（框架/基建用 core）；禁破折号与评审编号。署名：每会话首次提交前与用户明确（用户名+邮箱），无本地 git 配置则添加 local。默认自动提交（除非明确说不提交）；复杂模块开发前先提交基线；只 commit 不 push
- **并行多会话纪律**：可能有多个 agent 会话同时在本仓库工作；**只 `git add` 本会话改过的文件，禁止 `git add -A`/`git add .`**；动手前先 `git status` 看工作区有无他人进行中的改动（有则绕开，不同步不覆盖）；需要改他人未提交的文件时先与用户确认归属
- 任务书/设计文档落盘 `docs/`（插件级任务书放 `docs/plugins/<id>/`）
