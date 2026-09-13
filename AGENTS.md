# patchyBox · 项目简报（AGENTS.md）

> 本文件在会话打开 `G:\workspace\patchyBox` 时自动注入，先读它再动手。
> 本文只放**常青规则**；进度/历史看 git log 与 CHANGELOG.md，设计细节看 docs/。
>
> **动手写代码前，先读 [`docs/standards/10-AI开发工作流.md`](docs/standards/10-AI开发工作流.md)**：
> 它定义四档流程（S1 小改 → S4 新工具）、每档的确认门禁与验证命令、返修 3 轮上限，
> 以及"先定档再动手、发现超档必须升级"的硬要求。UI 改动另读
> [`docs/standards/11-插件UI开发约定.md`](docs/standards/11-插件UI开发约定.md)。

## 项目是什么

基于 **Tauri 2** 的桌面工具箱 **patchyBox**（Windows+macOS）。UI：**明净浅色 · 内容优先**（macOS 式侧栏 + 大卡片网格），原型见 `sketches/002-clean-light/`。现有工具：格式转换、接口调试、数据库、Hosts 编辑、DNS 解析、SSH 远程管理、文字转语音、FRP 配置管理、组件实验室（公共组件视觉验收页）。

## 已定技术决策（详见 docs/standards/01-技术选型.md）

- 容器：Tauri 2.11（Rust）+ 官方插件（clipboard-manager / global-shortcut / store / single-instance / autostart / notification / window-state / updater / dialog / fs / opener）
- 前端：**Vue 3.5 + TypeScript + Vite + Tailwind CSS 4 + Pinia** + fuse.js + vue-i18n（zh-CN 默认）
- 公共 UI：**shadcn-vue 源码模式 + Reka UI 无样式原语**，基础控件从 `@/core/ui` 使用 `Ui*`，凭证复合组件从 `@/core/vault`；tokens 以根目录 `DESIGN.md` 为单一事实源
- 明确禁用 `tauri-plugin-shell`。**产品能力必须覆盖 Windows 与 macOS**：公共契约两端一致、unsupported 必须可见；平台适配器允许使用原生依赖与经审查的 `unsafe`，不要求每个依赖本身跨平台（新增平台专用依赖按 docs/05 §8 评审并记录 target 条件）
- 凭据：系统 keyring 主密钥 + AES-256-GCM 凭证文件（`framework/vault`），插件可选引用 + 手工输入双路径

## 架构核心（详见 docs/standards/02-架构.md）

- **内置功能模块（以业务能力 owner 为边界）**：允许纯前端工具，也允许一个能力对应多个后端实现模块，**不要求前后端目录一一对应**。前端 `src/plugins/<owner>/`（manifest 自注册 + contracts/ + ipc.ts + 组件），Rust 侧 `src-tauri/src/plugins/<owner>/` 一律目录结构（mod.rs 门面 + models.rs + 能力子模块/子目录）；模块间**禁止互相 import 与直读对方内部状态/数据表**，协作走框架公开契约
- **框架与模块分离**：`src-tauri/src/framework/`（DataContext / paths / settings / vault 与 secure_store / PluginDb / 维护与关闭 / 命令元数据）是基建不属于任何模块；**删除死功能的依据是调用图与注册入口无消费方**（IPC 调用、动态入口、启动/恢复职责），不按“有没有前端同名目录”判断
- **IPC 接口入库**（`framework/module_manifest.rs` 静态清单 + `framework/ipc_registry.rs` 注册表）：每个模块在门面写一份 `patchybox_module!`（owner + featureId + `命令路径 => 中文说明`），同一标识符生成入库元数据与 handler（注册名 = 路径末段；兼容别名必须显式 `as "名"`）；`plugins/mod.rs` 的 `patchybox_routes!` 一行/模块生成路由、装配顺序与 `validate_routing()` 启动校验（重复注册、未纳路由 owner 均 fail-fast，不静默吞）。命令必须经清单入库，`framework/manifest_contract_tests.rs` 用源码扫描 + 已发布命令表契约测试拦截手写绕过
- **数据库管理规则**（`framework/store.rs`）：模块数据文件统一 `<storageRoot>/data/<owner>.db`，路径一律走 `framework::paths`，禁止手拼 `app_data_dir()`（四分区布局见 `docs/standards/02-架构.md` §3.1）；表结构走 `PRAGMA user_version` 顺序迁移，**只追加且以事务执行**，旧半迁移按已知版本与实际 schema 修复，不得把 `duplicate column` 整段当作成功跳过；统一骨架 `PluginDb`（连接生命周期 + 锁 + 迁移）
- **数据上下文与关闭**（`framework/context.rs` / `framework/lifecycle.rs`）：存储位置、空间代际与启动 epoch 只有一个来源，`paths` 与 `PluginDb` 消费同一实例，模块不得自拼物理路径或每次调用现读 `settings.json`；关闭只有 `lifecycle` 一个入口（`prepare` 可拒绝、`dispose` 有总超时），业务协议清理由各模块钩子提供，根迁移/导入提交/空间激活/更新安装共用 `context::maintenance_guard()`，前端禁用按钮不算锁
- **兼容边界**：向后兼容的对象是**已发布的数据格式与用户行为**，不是“所有框架源码只增不改”；内置模块之间可以协同重构（含目录、装配与状态所有权调整）
- **工具注册表**（`src/core/registry/`）：新增工具 = 建目录 + `plugins/index.ts` 一行；新增系统能力可以修改组装根、权限与生命周期注册，不承诺框架源码零改动
- **载体**：全部工具以多页签工作区子页面打开（`src/features/workspace/ToolWorkspace.vue`，页签支持 Ctrl+W 关闭、Ctrl(Shift)+Tab 循环、溢出三点收纳）
- **工具级设置**：manifest 声明 `settingsSchema`，框架自动渲染设置表单并存 `settings.tools[id]`

## 文档地图

| 文件 | 内容 |
| ---- | ---- |
| `docs/README.md` | **文档地图与批次索引：找文档先看这里** |
| `DESIGN.md` | 设计 tokens：28 色 / 18 组件变体（改后跑 `npx -y -p @google/design.md designmd lint DESIGN.md`） |
| `docs/standards/` | 常青规范与手册：01–09 为规范（技术选型 / 架构 / 模块开发规则 / 公共UI / Rust代码规范 / 发布 / 产品需求 / 术语与编号 / 平台能力矩阵），10–19 为 AI 开发手册（工作流 / 插件UI约定 / reka坑 / 表单排版 / 编辑器主题 / 测试与走查 / 多角色编排 / 文档组织 / 重构验证 / 契约审计，另有 templates/ 任务书模板） |
| `docs/adr/` | 跨批次架构决策（采纳后正文不改，只能被新 ADR 取代） |
| `docs/plugins/<id>/` | 插件的常青文档（需求 / 设计 / 界面设计 / 竞品分析） |
| `docs/batches/<批次号>/` | 任务性文档（任务书 / 执行计划 / 决策书 / 补证据），一批次一目录 |
| [`docs/进度台账.md`](docs/进度台账.md) | **进度唯一权威视图：每份任务书的执行状态与提交证据** |
| `TODO.md` | 跨会话待办总表：待办事项与用户裁决（进度看台账） |

**文档组织规则**：常青规范进 `docs/standards/`（中文名 + 两位序号），跨批次决策进 `docs/adr/`，插件常青文档进 `docs/plugins/<id>/`，任务性文档进 `docs/batches/<批次号>/`；批次号 = `<领域>-<YYYYMM>-<编号>-<中文简述>`（如 `ssh-202609-001-ssh工具`），**一个批次 = 一次开发**——开工先认领批次，进度只写该批次目录，跨批次关联靠批次号识别。命名与索引细则见 `docs/README.md`。

## 当前待办

**进度看 [`docs/进度台账.md`](docs/进度台账.md)**：每份任务书一行，标注 `✅ 已完成 / 🔶 部分完成 / ⬜ 未执行 / ⏸ 已裁决不补` 与提交证据，开工前查、收尾时更新（`python scripts/check_progress.py` 校验）。

**开工前的两份文档要求（用户 2026-09-13 定）**：
- **大任务必须先制定任务书和实现方案**——任务书写「做什么」（范围 / 产出 / 验收 / 明确不做什么），实现方案写「怎么做」（技术路线 / 改动面 / 步骤 / 风险）。两份都经用户确认后才动代码。
- **小任务直接改**：读代码 → 改 → 跑门禁 → 提交，不额外建文档。
- 档位判据见 [`docs/standards/10-AI开发工作流.md`](docs/standards/10-AI开发工作流.md)：S3 框架级与 S4 新工具必做两份文档，S1 直接改，S2 出根因分析（跨模块时升级）。

待办事项与用户裁决（明确不做 / 已搁置）仍在 `TODO.md`；本文件不保留状态清单。

## 关键设计约束（来自 DESIGN.md，务必遵守）

- 交互驱动色 `tertiary #F0562C`，小号白字按钮用 `tertiary-strong #C2410C`（WCAG AA）；选中态文字 tertiary-strong；HOT 标签 `success-strong #067647`
- 深色模式走 `-dark` token 变体，不新造色值；卡片网格 `auto-fill minmax(228px,1fr)`
- **暗色覆盖写 `main.css` 全局 unlayered 区**：组件 scoped 样式里 `:global()+:deep()` 混写会被编译静默丢弃

## 工程约束（合入红线）

- **规模是评审信号，不是自动失败条件**：Vue SFC 约 300 物理行、Rust 生产实现约 400 行、插件目录 8/15 个业务文件时**触发职责审查**（模板、生产逻辑、声明、测试分别统计）；超线须在提交说明写职责与拆分依据，无需每次申请例外。**必须重构的条件与行数无关**：多个独立修改原因、重复状态或重复实现、互传大批可写状态、资源释放无唯一所有者（细则 docs/03 §1）
- **Composable**：`useXxx` 管响应式状态与副作用，纯计算按 format/parse/reduce 等命名；非平凡规则、并发、迁移、取消与资源释放必须有测试，薄转发可由消费方测试覆盖
- **DTO**：每个 owner 一处权威定义（允许 `contracts/` 按域分文件）；Rust 模型与 TS 传输字段同步；内部 UI/表单状态不必复用 IPC DTO
- **锁与所有权**：同步锁不跨 `await`，确需跨 await 独占的 IO 用异步锁或「任务所有者 + 消息」；clone 按数据规模、频率与所有权评审，`Arc` 不是默认修法（细则 docs/05 §2/§4）
- **字体规范**：字号只用语义 token（text-h1/text-brand/text-card-title/text-h2/text-body/text-body-sm/text-caption/text-label-caps/text-display），禁 arbitrary `text-[*px]`；字体族 `--font-sans`/`--font-mono`（font-mono 只用于明文数据/代码内容，密码框不加）
- **公共组件**：避免各插件自绘 UI 差异——密码框统一 `UiInput type="password"`（自带眼睛）、下拉 `UiSelect`、可搜索下拉 `UiCombobox`、列表行 `UiListRow`、右键菜单 `ContextMenu`、删除确认 `ConfirmDialog`、页签溢出 `UiTabsOverflow`；高度基线 36px
- **反馈与错误**：显式操作必须可感知结果（toast / 错误行 / 状态变化 / 加载指示都算反馈），禁止静默 catch；取消与预期缺失不弹错误；IO、解析与密钥失败不得伪装成功
- **注释**：强制模块职责、公共契约、IPC 字段的单位/空值/敏感性、关键生命周期与安全不变量；私有显然函数与测试不要求复述名字，注释质量由人工审查（细则 docs/05 §9）
- **验证按风险分层**（矩阵见 `docs/batches/arch-202609-001-架构重构与规范调整/架构重构与工程规范调整任务书.md` §12）：纯文档查引用与格式；前端逻辑跑相关测试 + `pnpm lint` + `pnpm build`；Rust 逻辑跑相关测试 + `cargo fmt` + `clippy -- -D warnings`；持久化/凭证/上下文加旧数据夹具与故障恢复；权限/注册/Cargo 运行依赖加双平台构建与冷启动。**本地只格式化本次改动的文件，CI 用 `pnpm format:check`**；发布与跨层合入跑全量门禁
- **dev 冷启动冒烟**：改 `tauri.conf.json`/`lib.rs` 插件注册/`Cargo.toml`/能力权限时，提交前必须 `pnpm tauri dev` 冷启动确认无 panic（教训：updater 配置缺失曾致 dev 启动崩两周无人发现）
- **Tauri 已知坑**：`dragDropEnabled`（默认开）会吞掉应用内 HTML5 拖拽——内部拖拽用 pointer 事件自实现（参照 `src/plugins/ssh/useServerGroups.ts` 的 `useGroupDrag`）

## 环境备忘（Windows）

- **pnpm**：系统 corepack shim 损坏，先 `export PATH="/c/Users/patchy/AppData/Roaming/npm:$PATH"`（npm 全局 pnpm）再执行 pnpm；pnpm 11 配置在 `pnpm-workspace.yaml`（package.json 的 pnpm 字段已废弃）
- 所有命令在 git-bash 执行；Windows 环境
- cmdkey 输出为 GBK 且含 NUL，需 `iconv -f GBK -t UTF-8` 后 grep

## 约定

- 文档与注释中文；代码标识符英文
- **提交**：Conventional Commits `类型(scope): 中文描述`；标题一行总概括，body 按代码增删改分条、内容具体；类型按实质（搬移 refactor/隐患 fix/新机制 feat/测试 test）、scope 按真实改动模块（框架/基建用 core）；禁破折号与评审编号。署名：每会话首次提交前与用户明确（用户名+邮箱），无本地 git 配置则添加 local。默认自动提交（除非明确说不提交）；只 commit 不 push
- **提交信息自包含（2026-09-06 用户重申）**：读标题就必须知道「改了什么行为 / 解决了什么问题」，禁止依赖外部上下文（任务书、issue、会话记录）才能看懂——禁止「整改任务书必修 8」「完成 XX 验证」「旗舰能力」这类元描述。反面：`fix(ssh): 整改任务书必修 8 + 中危 7 + 低危 5 全量行为修复`（等于没说）；正面：`fix(ssh): 重连成功保留终端缓冲（sessionId 变化不再触发 xterm reset）`（问题 + 机制一目了然）。一个提交修多个 bug 时，标题写最重要的那个，body 逐条列「现象 → 修法」。
- **基线纪律**：复杂模块开发前先记录起始 HEAD，只提交自己已有完整改动；工作区干净时不造空基线提交；有他人未提交改动时不暂存其文件
- **并行多会话纪律**：可能有多个 agent 会话同时在本仓库工作；**只 `git add` 本会话改过的文件，禁止 `git add -A`/`git add .`**；动手前先 `git status` 看工作区有无他人进行中的改动（有则绕开，不同步不覆盖）；需要改他人未提交的文件时先与用户确认归属
- **文档落位**（2026-09-13 重整后）：框架规范与 AI 开发手册进 `docs/standards/`，跨批次决策进 `docs/adr/`，插件常青文档进 `docs/plugins/<id>/`，任务性文档进 `docs/batches/<批次号>/`；索引见 `docs/README.md`
- **AI 开发手册在仓库里，不在 AI 助手侧**：流程、UI 约定、已知坑、验证方法都是仓库文档（`docs/standards/10-` 起）。换 AI 工具或换机器时，手册必须仍然可用——不要把项目知识只写进外部技能库
