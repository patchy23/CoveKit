# patchyBox · 模块开发规则（v2 · 2026-09-12）

> 本文件是大规模开发前的**规则体系**：几十上百个工具将按此规则开发，避免后期重构。
> 与 `01-tech-stack.md`、`02-architecture.md` 并列；规则冲突时以本文为准并回改前两文。
> v2 修订（2026-09-12，依据《架构重构与工程规范调整任务书》AR01）：模块以业务能力 owner 为边界；文件规模改为评审信号；composable 与注释规则按语义而非形态；锁与 clone 按成本判断；验证按风险分层。

## 0. 总则（铁律）

1. **模块以 owner 为边界**：一个业务能力对应一个 owner（前端 `src/plugins/<owner>/` + Rust `src-tauri/src/plugins/<owner>/`）；前端与后端模块数**不要求一一对应**——纯前端工具没有 Rust 模块，一个能力也可以有多个后端实现模块。模块间**禁止互相 import、禁止直读对方内部状态与数据表**，跨能力协作走框架公开契约。
2. **兼容边界是数据与用户行为**：向后兼容的对象是**已发布的数据格式与用户行为**；内置模块之间可以协同重构（目录、装配、状态所有权），不承诺「框架源码只增不改」「已发布插件对框架零依赖」。
3. **公共能力按层下沉**：基础控件与编辑器走 `src/core/ui/`，凭证复合 UI 走 `src/core/vault/`，剪贴板/窗口等平台操作走 `src/core/platform/`，应用级通知走 `src/core/feedback/`，IPC 基础设施走 `src/core/ipc/`，数据与持久化基建走 `src-tauri/src/framework/`；模块不得重复造轮子（各自实现 toast、各自建数据库、各自解析存储根）。基础层不得反向依赖 `stores`、业务 IPC 或其它模块内部实现。
4. **接口入库**：所有 Tauri 命令必须在启动时登记到 IPC 注册表（见 §2），重复注册直接报错。
5. **编辑器统一**：多行代码 / 配置编辑一律用 `@/core/ui` 的 `UiCodeEditor`，差异对比用 `UiCodeDiff`；**禁止自研编辑器、禁止复制 CodeMirror 主题**（详见 §7）。

## 1. 目录与命名规范

**所有模块一律目录结构（无单文件例外）**：

```
前端  src/plugins/<owner>/              Rust  src-tauri/src/plugins/<owner>/
├── index.ts      注册 manifest          ├── mod.rs   门面（命令薄层 + register/init + mod models;）
├── index.vue     主组件（规模见下）      ├── models.rs serde 结构（字段 pub(crate)，与前端契约同步）
├── contracts.ts  本模块 IPC 契约        └── <能力>.rs 或 <能力>/ 按能力域拆分（多实现变体用 trait 或封闭枚举）
│   （契约较多时改 contracts/ 分域文件）
├── ipc.ts        命令封装（invokeCommand）
├── useXxx.ts     状态与副作用（复杂规则必须测试）
└── 子组件 / 纯函数 / 测试（按特性域分子目录）
```

- `id`：小写连字符（`http-ws`、`random-password`）；Rust 模块名 snake_case（`http_ws`）。
- 命令名：snake_case（`db_open`）；前端封装名 camelCase（`dbOpen`）。
- 字段：serde 统一 `camelCase`；错误结构统一 `{ ok: false, error: Option<String> }`（不抛错给前端展示）。
- 每个插件在 `mod.rs` 提供自己的 `invoke_handler(invoke)`，内部 `generate_handler!` 使用完整路径；应用级 Builder 只安装一次总 handler，由 `plugins/mod.rs` 按注册表 owner 精确路由（2026-09-06 起；旧的前缀手写清单已废弃）。
- **禁止在多个 `register()` 中调用 `Builder::invoke_handler`**：该方法是 setter，后调用会覆盖前一批命令，并非追加。
- **规模是评审信号，不是自动失败条件（2026-09-12 修订）**：Vue 文件约 300 物理行、Rust 生产实现约 400 行、插件目录 8（Rust 能力文件）/15（前端文件）个业务文件时**触发职责审查**——模板、生产逻辑、声明表与测试分别统计。超线时在提交说明写清职责与拆分依据即可，不需要每次向用户申请例外；**行数是线索不是目标，禁止用复制实现、挪进巨型 use 文件或过度转发来满足数字**。
- **必须重构的条件（与行数无关，命中即处理）**：同一文件存在多个独立修改原因；同一份状态或实现存在两处；模块之间互传大批可写状态；资源释放没有唯一所有者。反之，职责单一、状态所有者清晰的较长文件可以保留并说明理由。
- **目录按能力域组织**：新增状态域模块直接建在对应子目录（如 `ssh/profiles/`、`database/catalog/`），不为凑文件数搬家；当一个插件目录难以导航或单域文件明显过多时做归域，用 `git mv` 保历史，门面层（index.ts / index.vue / contracts / ipc.ts）留在插件根，跨域共享只对确有多个消费方的模块设明确入口，不建杂项 `utils` 堆放区。目录分层是职责调整，不恢复全仓文件数量硬门禁。
- **模块边界不靠目录同名判断**：删除死功能的依据是调用图、注册入口（IPC 调用、动态入口、启动/恢复职责）无消费方，而不是「没有前端同名目录」。
- 框架级能力（设置/快捷键/窗口/命令入库/数据管理）在 `src-tauri/src/framework/`，**不属于插件**。

## 2. IPC 接口入库规则（tauri 接口入库）

每个插件在 `register()` 中登记命令清单：

```rust
pub fn register(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    // 第一个参数是插件 id（owner）：路由按注册表精确匹配命令名，不再靠手写前缀
    ipc_registry::register("db", &[
        ("db_open", "打开数据库（路径不存在自动创建）"),
        ("db_execute", "执行 SQL（查询/非查询自动识别）"),
    ]);
    builder.manage(...)
}
```

规则：
- **命令名全局唯一**：启动时 `ipc_registry` 检测重复，重复即 panic（开发期暴露，杜绝两个插件抢命令名）。
- **唯一总 handler**：`lib.rs` 只调用一次 `Builder::invoke_handler`；`plugins/mod.rs` 的 `invoke_handler` 按注册表 owner 分派（新增插件加一行 `Some("<id>") => <id>::invoke_handler(invoke)` 分支），`validate_routing()` 启动期 fail-fast 校验「登记了但没路由分支」。
- **入库元数据**：`(名称, 中文说明)` 是入库最小单位；说明必须写清用途与关键参数。
- **可查询**：框架命令 `framework_commands` 返回全量清单（名称 + 说明），供前端调试面板/文档生成。
- **契约同步**：前端 `contracts.ts` 与 Rust serde 结构逐字段对应；`rename_all = "camelCase"` 是默认，禁止手写不一致。每个 owner 的传输 DTO 只有一处权威定义（契约多时按域分文件），内部 UI/表单状态不必复用 IPC DTO。
- **owner 与 feature id 的映射显式声明**：注册 owner 用 Rust 模块名（snake_case），产品 feature id 用前端 id（如 `http-ws`）；一个 feature 调用多个 owner 的命令时，映射写在模块描述里，不靠字符串拼接或前缀猜测。
- **入参校验**：命令内自行校验（URL 格式、SQL 白名单等），失败返回 `Err(String)` 或 `ok:false` 结构，禁止 panic。

## 3. 数据库操作管理规则

### 3.1 文件与路径约定

- 插件数据文件统一存放：`<storageRoot>/data/<plugin-id>.db`（默认 `%APPDATA%/com.patchy23.patchybox`，macOS `~/Library/Application Support/...`）。
- **路径获取一律走 `framework::paths`，禁止手拼 `app_data_dir()` 或任何绝对路径**：
  - `data_path(app, "<name>")` 数据分区文件（插件数据库 `plugin_db_path` 即其封装，另有回落旧布局的语义）
  - `vault_dir(app)` 凭证密文分区；`logs_dir_for(app, scope)` 日志目录；`cache_dir(app, scope)` 可重建缓存
  - 数据根目录可由用户在设置页「存储位置」修改（四分区布局与迁移流程见 `docs/02-architecture.md` §3.1）
- 默认**每数据空间 / 持久化 owner 一个数据文件**；历史文件名（如接口收藏的 `api.db`）以显式 `storageKey` 映射保留，不为统一命名迁移或清空用户数据。跨 owner 共享数据必须经框架（当前不允许，后续需要时新增框架 API）。

### 3.2 引擎选择

| 场景 | 引擎 | 说明 |
|---|---|---|
| 本地键值/记录（接口列表、剪贴板历史、设置） | **rusqlite（bundled）** | 统一走框架骨架 **`PluginDb`**（`framework::store`：路径 + 迁移 + 锁内访问），插件只写业务 SQL |
| 连接型数据库调试（MySQL/PostgreSQL/Redis/SQLite/Oracle 及国产库） | **原生驱动**：`mysql_async` / `tokio-postgres` / `redis` / `rusqlite`；Oracle、Vastbase、Kingbase 走 `database/agent` | 传输归 `drivers/`，SQL 语义归 `dialect/`；sqlx 曾预留、实际未使用，已于 2026-09-06 移除 |
| 复杂全文检索 | rusqlite FTS5 | 同 rusqlite（经 `PluginDb::with_conn`） |

**PluginDb 用法**（本地库插件标准姿势，禁止自建连接样板）：

```rust
pub struct XxxState(pub Mutex<Option<PluginDb>>);   // 惰性 State
const MIGRATIONS: &[&str] = &["CREATE TABLE ...;"];  // 只追加

// 命令内：
let guard = db(&app, &state)?;                      // 锁内借用（首次自动打开+迁移）
guard.as_ref().unwrap().with_conn(|c| {
    // rusqlite 全 API，业务 SQL 自由书写
    c.execute("INSERT ...", params).map_err(|e| e.to_string())?;
    Ok(())
})
```

边界：**不做 ORM/查询构造器**——复杂查询（join/聚合/FTS）直接在 `with_conn` 闭包写 SQL；连接型（sqlx）场景不套 PluginDb。

### 3.3 迁移规则

- 表结构变更用 `PRAGMA user_version` 版本号 + 顺序迁移数组：

```rust
pub fn migrate(conn: &Connection, migrations: &[&str]) -> Result<(), String> {
    let cur: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);
    for (i, sql) in migrations.iter().enumerate().skip(cur as usize) {
        conn.execute_batch(sql)?;
        conn.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
    }
    Ok(())
}
```

- **只允许追加迁移**，禁止修改已发布迁移；结构变更 = 追加一条。
- **迁移以事务执行**（DDL 与 `PRAGMA user_version` 同批提交），中途失败时按已知版本与实际 schema 修复；**不得把 `duplicate column` 之类的单个报错当成「整段迁移已成功」而跳到下一版**。
- 新建表必须走迁移数组（第 0 版），不允许散落 `CREATE TABLE IF NOT EXISTS` 于业务代码。

### 3.4 连接生命周期

- 连接用 `Mutex<Option<Connection>>` 惰性初始化（State 注入），命令内**锁内同步执行，锁不跨 await**；确需跨 await 独占资源的场景用异步锁或「任务所有者 + 消息」，不按锁的类型名选锁。
- 连接型会话（原生驱动连接池 / SSH / WS 句柄）生命周期归各自模块管理（open/close 命令），其它模块不得直接持有。

## 4. 工具架构设计（复杂工具模板）

复杂工具（SSH、DNS、DB、云端服务）统一分层：

```
前端（Postman 式参考）：
  主组件（容器：状态协调 + 布局；规模按 §1 评审信号判断）
  ├── 侧栏/列表组件（数据集合管理）   接收数据 + emit 操作
  ├── 编辑面板（表单/请求构建）       受控组件（props + emit）
  ├── 结果查看组件（响应/日志）       纯展示
  useXxx.ts：状态与副作用（连接生命周期/消息队列）；复杂规则、并发与释放路径必须测试，纯计算拆独立命名文件

Rust：
  mod.rs     命令薄层（参数校验 → 调服务层）+ register/init
  models.rs  serde 结构
  <能力>.rs  服务层：会话注册表（State）、后台任务、适配器
  多实现变体：trait + 子模块（如 db/dialect.rs + sqlite.rs/mysql.rs/postgres.rs）
```

以 SSH 工具为例（M3 预留形状）：
- 会话：`SshState(Mutex<HashMap<String, SshSessionHandle>>)`（与 http_ws 的 WsState 同构）
- 命令：`ssh_connect / ssh_exec / ssh_sftp_* / ssh_close / ssh_sessions`
- 凭据：不落明文，引用 `ConnectionProfile.secretRef`（stronghold，M3）
- 前端：连接列表侧栏 + 终端/命令面板（复用 Select/CodeViewer 等 core/ui）

## 5. 质量门槛（合入红线）

1. **验证按风险分层（2026-09-12 修订）**：纯文档改动查引用与格式即可；前端逻辑跑相关测试 + `pnpm lint` + `pnpm build`（含 typecheck）；Rust 逻辑跑相关测试 + `cargo fmt` + `clippy -- -D warnings`；持久化 / 凭证 / 上下文改动加旧数据夹具与故障恢复用例；权限、注册与 Cargo 运行依赖改动做双平台构建与冷启动。**本地只格式化本次改动的文件，CI 用 `pnpm format:check` 做全仓只读检查**；发布与跨层合入必须跑全量门禁（矩阵见《架构重构与工程规范调整任务书》§12）。
2. **测试按行为价值**：非平凡规则、并发、失败与迁移路径、取消与资源释放必须有测试；纯计算按独立命名文件配单测；薄转发（只是调用下游）可由消费方测试覆盖，不为满足「同名测试文件」配额造断言。
3. **规模与拆分**：按 §1 的评审信号与「必须重构的条件」判断，行数本身不构成自动失败。
4. **反馈与错误**：显式操作必须可感知结果（toast / 错误行 / 状态变化 / 加载指示都算反馈），禁止静默 catch；取消与预期缺失不弹错误；IO、解析与密钥失败不得伪装成默认成功。
5. 提交信息：conventional commits 带模块作用域（`fix(http-ws): ...`；框架用 `core`）。
6. **代码注释（按语义强制，2026-09-12 修订）**：必须写的是——(a) 文件头模块职责（`//!`）；(b) `pub`/`pub(crate)` API 用途；(c) IPC 与持久化 DTO 的非显然字段（单位 / 空值语义 / 敏感性）；(d) 关键生命周期、锁与安全不变量（`unsafe` 必带紧邻 `// SAFETY:`）。私有且语义显然的函数、测试辅助不强制也不扣分；注释质量由人工审查，工具只做覆盖检查（`python scripts/check_docs.py`，AR02 起实现为 `src-tauri/tests/source_rules` 的 syn 扫描：文件头、pub/pub(crate)/pub(super) 项含固有 impl 方法、pub 结构体字段；枚举变体与 trait 实现关联项不强制）。
7. 前端通用控件必须从 `@/core/ui` 导入，禁止在插件内复制按钮、表单、页签、面板、弹窗和状态反馈样式；完整规范见 `docs/04-ui-components.md`。
8. **Rust 代码规范**（`docs/05-rust-code-standard.md`）：提交前跑 `python scripts/check_rust_rules.py` 与 `python scripts/check_docs.py`（两者是 `cargo test --test source_rules` 的薄 wrapper，CI 跑同一 target）；运行期禁 unwrap/expect（例外须按路径+符号+类别登记在基线文件里）、clone 按规模与所有权评审、`unsafe` 须必要且带紧邻 `// SAFETY:` 论证。
9. **dev 冷启动冒烟（2026-09-06 新增，血泪教训：b75b531 注册 updater 插件但基座配置缺段，之后两周 dev 启动即 panic 无人发现）**：改动涉及 `tauri.conf.json` / `lib.rs` 插件注册 / `Cargo.toml` 依赖 / 能力权限时，提交前必须 `pnpm tauri dev` 冷启动一次，确认窗口正常出现、控制台无 panic。只改前端或纯逻辑可豁免。
10. **Tauri 已知坑**：`dragDropEnabled`（默认开，OS 文件拖入依赖它）会吞掉应用内 HTML5 拖拽——内部拖拽一律用 pointer 事件自实现（参照 `src/plugins/ssh/useServerGroups.ts` 的 `useGroupDrag`）；SFC scoped 样式里 `:global()+:deep()` 混写会被编译静默丢弃，暗色覆盖写 `main.css` 全局 unlayered 区。
11. **前端依赖守卫（AR02 新增）**：提交前跑 `pnpm check:deps`（`scripts/check_frontend_deps.mjs`）——`core/ui` 不得依赖 `stores`/`features`/`plugins`/`vault` 与应用 IPC；插件之间不得 import 对方内部模块（含类型导入）；`core/ui` 入口不得重导出 vault/插件模块；`src/core/**` 运行期依赖图不得有环（`import type` 不算运行期）。存量违规登记在 `scripts/frontend_deps_baseline.json`（棘轮，AR03 逐步清零）：出现未登记违规、或登记条目已不再命中（失效例外）即失败。

## 6. 新增插件 Check-list

- [ ] 前端 `src/plugins/<id>/`：manifest + contracts.ts + ipc.ts + index.vue + useXxx.ts + 测试
- [ ] Rust `src-tauri/src/plugins/<owner>/`（**目录形式，无单文件例外**）：`mod.rs` 门面 + `models.rs` + 能力文件（或能力子目录）；命令 + register（含 ipc_registry 入库）+ 需要时 init
- [ ] `plugins/mod.rs` 一行 + `lib.rs` register/init 各一行 + 前端 `plugins/index.ts` 一行
- [ ] 数据文件走 `framework::store`；表结构走迁移数组
- [ ] 命令在 `ipc_registry` 登记；契约 camelCase 同步
- [ ] 页面通用控件使用 `@/core/ui`，并在“组件实验室”核对亮色/深色和交互状态
- [ ] 文本编辑区域使用 `UiCodeEditor`（只读用 `readonly`，差异用 `UiCodeDiff`），未自研编辑器 / 未复制 CodeMirror 主题（§7）
- [ ] 全量验证（§5）通过后提交（conventional commits + 插件作用域）

## 7. 编辑器与文本输入规范（2026-09-11 起）

背景：编辑器功能曾散落成 3 处自研 CodeMirror 包装（`CodeViewer`、`SqlEditor`、`EditorDialog`）
与 7 处自研「行号 + 文本域」（`LineNumberTextarea`），导致每个工具的高亮配色、快捷键、
降级策略各不相同。现统一为 `core/ui` 的编辑器组件，**新增代码不得再出现第二套实现**。

| 场景 | 用法 |
| --- | --- |
| 单行输入 | `UiInput` |
| 多行纯文本（无需行号/高亮/查找） | `UiTextarea` |
| 多行代码、配置、日志（行号 + 高亮 + 缩进） | `UiCodeEditor mode="minimal"` |
| 完整编辑（查找替换 / 格式化 / 校验 / 补全 / 状态栏） | `UiCodeEditor`（默认 `full` 档） |
| 只读查看 | `UiCodeEditor readonly`（原 `CodeViewer` 已删除） |
| 差异对比 | `UiCodeDiff`（`split` 左右对照 / `unified` 内联并可接受或拒绝） |

硬约束：

1. **主题唯一源**：`core/ui/editor/theme.ts` + `main.css` 的 `--cm-*` 变量；暗色覆盖写 `main.css` 全局 unlayered 区，不得在插件里重新定义高亮样式。
2. **领域能力靠注入**：插件专有扩展（例如数据库的「执行当前语句」gutter、Ctrl+点击表名跳转、SQL 方言）
   通过 `languageExtension` / `extraExtensions` / `completionSources` 传给编辑器，
   **不得在插件内自建 `EditorView` 或自己挂主题**（`src/plugins/database/SqlEditor.vue` 是范例：137 行薄封装）。
3. **大文件与降级交给组件**：>512KB 自动关闭高亮、折叠与补全，>5MB 强制只读并提示；插件不要自行判断与降级。
4. **封装组件只做领域适配**：插件侧编辑器封装限于 props 映射与命令式 API 转发，通用能力一律回上游 `core`；规模按 §1 的评审信号判断，逼近时优先外置领域逻辑，不得复制公共实现。

**契约守卫（防回归）**：`src/core/ui/editor/editorUnification.test.ts` 扫描全仓库源码并断言以下四条，`pnpm test` 变红即说明有工具绕过了公共组件：

1. 除 `UiCodeEditor.vue` / `UiCodeDiff.vue` / `editor/` 之外，任何文件不得实例化编辑器（`new EditorView` / `EditorState.create` / `new EditorState` / `monaco.editor.create` / `ace.edit`）；
2. 不得引入其它编辑器基座（`monaco-editor` / `ace-builds` / `quill` / `@tiptap/*` / `prosemirror-*` / `@milkdown/*` / `vditor` / `codemirror` v5）；
3. 工具层（`plugins/` `features/`）不得直接 `import` 编辑器内部实现（`@/core/ui/editor/*`），只能用 `@/core/ui` 的公共组件；
4. 不得残留已删除的自研编辑器（`LineNumberTextarea` / `CodeViewer`）。

**允许的例外**：插件向**共享编辑器注入扩展**不受限制（如数据库插件注入 SQL 补全、语句运行 gutter 与表名跳转，见 `plugins/database/sqlEditorExtensions.ts`）——契约禁的是「自己造一个编辑器」，不是「给统一编辑器加领域能力」。

