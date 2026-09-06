# patchyBox · 插件开发规则（v1 · 2026-08-08）

> 本文件是大规模开发前的**规则体系**：几十上百个工具将按此规则开发，避免后期重构。
> 与 `01-tech-stack.md`、`02-architecture.md` 并列；规则冲突时以本文为准并回改前两文。

## 0. 总则（铁律）

1. **插件自包含**：前端插件目录 `src/plugins/<id>/` 与 Rust 插件 `src-tauri/src/plugins/<id>` 一一对应（同名同边界），插件间**禁止互相 import / 互相调用**。
2. **只增不改**：已发布插件对框架与其它插件零依赖变更；框架升级不允许破坏既有插件。
3. **公共能力下沉框架**：组件走 `src/core/ui/`，IPC 基础设施走 `src/core/ipc/`，数据层走 `src-tauri/src/framework/`；插件不得重复造轮子（如各自实现 toast、各自建数据库）。
4. **接口入库**：所有 Tauri 命令必须在启动时登记到 IPC 注册表（见 §2），重复注册直接报错。

## 1. 目录与命名规范

**所有插件一律目录结构（无单文件例外）**：

```
前端  src/plugins/<id>/                 Rust  src-tauri/src/plugins/<id>/
├── index.ts      注册 manifest          ├── mod.rs   门面（命令薄层 + register/init + mod models;）
├── index.vue     主组件（≤300 行）       ├── models.rs serde 结构（字段 pub(crate)，与前端 contracts.ts 同步）
├── contracts.ts  本插件 IPC 契约        └── <能力>.rs 按能力拆分（如 http.rs / ws.rs；多实现变体用 trait）
├── ipc.ts        命令封装（invokeCommand）
├── useXxx.ts     纯函数（必须带单测）
└── 子组件 / 测试
```

- `id`：小写连字符（`http-ws`、`random-password`）；Rust 模块名 snake_case（`http_ws`）。
- 命令名：snake_case（`db_open`）；前端封装名 camelCase（`dbOpen`）。
- 字段：serde 统一 `camelCase`；错误结构统一 `{ ok: false, error: Option<String> }`（不抛错给前端展示）。
- 每个插件在 `mod.rs` 提供自己的 `invoke_handler(invoke)`，内部 `generate_handler!` 使用完整路径；应用级 Builder 只安装一次总 handler，由 `plugins/mod.rs` 按注册表 owner 精确路由（2026-09-06 起；旧的前缀手写清单已废弃）。
- **禁止在多个 `register()` 中调用 `Builder::invoke_handler`**：该方法是 setter，后调用会覆盖前一批命令，并非追加。
- **Rust 文件规模红线（2026-09-06 新增，教训：ssh 插件平铺 14 个 rs / conn.rs 1222 行）**：单个 rs 文件 >400 行、或插件目录下能力文件 >8 个时，必须按能力域下沉子目录（如 `ssh/conn/`、`ssh/sftp/`、`ssh/ops/`），子目录 `mod.rs` 用 `pub use` 重导出保持 `crate::plugins::<id>::<域>::xxx` 引用路径稳定，调用方零改动；门面 `mod.rs` 保持薄（命令薄层 + register/init + 模块声明）。
- **前端目录规模红线（2026-09-06 新增，与 Rust 侧对称）**：插件目录下文件 >15 个（不含测试）时，必须按特性域分目录（如 `ssh/connection/`、`ssh/terminal/`、`ssh/files/`），门面层（index.ts/index.vue/contracts.ts/ipc.ts）留在插件根；子目录内组件用相对路径引用，跨域共享逻辑放 `shared/`；移动文件用 `git mv` 保历史。
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
- **契约同步**：前端 `contracts.ts` 与 Rust serde 结构逐字段对应；`rename_all = "camelCase"` 是默认，禁止手写不一致。
- **入参校验**：命令内自行校验（URL 格式、SQL 白名单等），失败返回 `Err(String)` 或 `ok:false` 结构，禁止 panic。

## 3. 数据库操作管理规则

### 3.1 文件与路径约定

- 插件数据文件统一存放：`%APPDATA%/com.patchy23.patchybox/<plugin-id>.db`（macOS `~/Library/Application Support/...`）。
- 路径获取走框架助手 `framework::store::plugin_db_path(app, "api")`，禁止插件手拼路径。
- 每个插件**一个数据文件**；跨插件共享数据必须经框架（当前不允许，后续需要时新增框架 API）。

### 3.2 引擎选择

| 场景 | 引擎 | 说明 |
|---|---|---|
| 本地键值/记录（接口列表、剪贴板历史、设置） | **rusqlite（bundled）** | 统一走框架骨架 **`PluginDb`**（`framework::store`：路径 + 迁移 + 锁内访问），插件只写业务 SQL |
| 连接型数据库调试（SQLite/MySQL/PG） | **sqlx 0.8** | 三方言统一，`DbDialect` trait 按方言实现（M3） |
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
- 新建表必须走迁移数组（第 0 版），不允许散落 `CREATE TABLE IF NOT EXISTS` 于业务代码。

### 3.4 连接生命周期

- 连接用 `Mutex<Option<Connection>>` 惰性初始化（State 注入），命令内**锁内同步执行**，禁止跨 await 持锁。
- 连接型（sqlx Pool）生命周期归 db 插件管理（open/close 命令），其它插件不得直接持有。

## 4. 工具架构设计（复杂工具模板）

复杂工具（SSH、DNS、DB、云端服务）统一分层：

```
前端（Postman 式参考）：
  主组件（容器：状态中枢 + 布局）     ≤300 行
  ├── 侧栏/列表组件（数据集合管理）   接收数据 + emit 操作
  ├── 编辑面板（表单/请求构建）       受控组件（props + emit）
  ├── 结果查看组件（响应/日志）       纯展示
  useXxx.ts：纯函数 + 状态机（连接生命周期/消息队列） 必须单测

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

1. `pnpm lint --max-warnings 0` + `pnpm format`（CI 有 format:check，提交前必须本地跑过）+ `pnpm test` + `pnpm build` 全绿；Rust `clippy -D warnings`（`--no-default-features`）+ `fmt --check` + `cargo test` 全绿。
2. 纯函数必须单测（`useXxx.test.ts`）；契约字段变更必须同步更新测试。
3. 组件 ≤300 行；逻辑抽 `useXxx.ts`。
4. 所有用户操作必须有可见反馈（toast/错误行），禁止静默 catch。
5. 提交信息：conventional commits 带插件作用域（`fix(http-ws): ...`；框架用 `core`）。
6. **代码注释（Rust 强制）**：每个 rs 文件必须有文件头注释（`//!` 说明模块职责）；文件内每个结构体/函数必须有 `///` 用途注释；**结构体每个属性必须注释其作用**（`/// 字段含义`）；函数内关键逻辑/复杂结构（锁、异步任务、迁移、提权等）必须加行注释（`// 说明`）。提交前用 `scripts/check_docs.py` 复查覆盖率。
7. 前端通用控件必须从 `@/core/ui` 导入，禁止在插件内复制按钮、表单、页签、面板、弹窗和状态反馈样式；完整规范见 `docs/04-ui-components.md`。
8. **Rust 代码规范**（`docs/05-rust-code-standard.md`）：提交前跑 `python scripts/check_rust_rules.py`（棘轮基线只减不增）；运行期禁 unwrap/expect、clone 需答所有权、unsafe 零容忍。
9. **dev 冷启动冒烟（2026-09-06 新增，血泪教训：b75b531 注册 updater 插件但基座配置缺段，之后两周 dev 启动即 panic 无人发现）**：改动涉及 `tauri.conf.json` / `lib.rs` 插件注册 / `Cargo.toml` 依赖 / 能力权限时，提交前必须 `pnpm tauri dev` 冷启动一次，确认窗口正常出现、控制台无 panic。只改前端或纯逻辑可豁免。
10. **Tauri 已知坑**：`dragDropEnabled`（默认开，OS 文件拖入依赖它）会吞掉应用内 HTML5 拖拽——内部拖拽一律用 pointer 事件自实现（参照 `src/plugins/ssh/useServerGroups.ts` 的 `useGroupDrag`）；SFC scoped 样式里 `:global()+:deep()` 混写会被编译静默丢弃，暗色覆盖写 `main.css` 全局 unlayered 区。

## 6. 新增插件 Check-list

- [ ] 前端 `src/plugins/<id>/`：manifest + contracts.ts + ipc.ts + index.vue + useXxx.ts + 测试
- [ ] Rust `src-tauri/src/plugins/<id>.rs`：命令 + register（含 ipc_registry 入库）+ 需要时 init
- [ ] `plugins/mod.rs` 一行 + `lib.rs` register/init 各一行 + 前端 `plugins/index.ts` 一行
- [ ] 数据文件走 `framework::store`；表结构走迁移数组
- [ ] 命令在 `ipc_registry` 登记；契约 camelCase 同步
- [ ] 页面通用控件使用 `@/core/ui`，并在“组件实验室”核对亮色/深色和交互状态
- [ ] 全量验证（§5）通过后提交（conventional commits + 插件作用域）
