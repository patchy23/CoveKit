# patchyBox · 待办总表（跨会话）

> 由 `AGENTS.md` 指向，开工前先读本表；完成一项勾掉一项，全部完成后清空本表相关小节。
> 本表只记「有什么要做、做到哪一步」，实施细则看对应任务书。

---

## 一、✅ 已完成 · 公共编辑器组件（2026-09-11 四批全部落地）

任务书：`docs/tasks/2026-09-11-编辑器组件任务书.md`（契约已冻结，含 22 项功能清单与性能预算）

背景：工具内编辑器现状是两套不成熟实现——自研 `LineNumberTextarea`（7 处调用）+ 手搓 CodeMirror（3 处，主题重复定义）。用户要求做成一个**高性能、低占用、功能齐全**的公共组件，供后续所有工具引用。

选型：**CodeMirror 6 为唯一基座**（Monaco 解包 93.4MB + 依赖 web worker 需放宽 CSP，与单 exe 便携交付冲突）。用户已确认这一选型。

| #   | 批次 | 内容                                                                                                        | 状态       |
| --- | ---- | ----------------------------------------------------------------------------------------------------------- | ---------- |
| 1   | 批 1 | 骨架：`UiCodeEditor.vue` + `editor/{useCodeEditor,extensions,languages,theme}.ts` + minimal 模式 + showcase | **已完成** |
| 2   | 批 2 | 高级能力：中文查找替换 / 快捷键 / 状态栏 / 格式化 / 错误标记 / 补全注入 / 大文件降级 / 右键菜单             | **已完成** |
| 3   | 批 3 | 迁移（3 处手搓 CM6 + 6 处 LineNumberTextarea）并**删除自研组件**                                            | **已完成** |
| 4   | 批 4 | `UiCodeDiff`（diff）+ SSH 冲突差异接入 + 文档与体积/内存实测                                                | **已完成** |

批 1 完成内容（2026-09-11）：`UiCodeEditor` 对外组件 + `editor/` 五模块（含缩进参考线 `indentGuides.ts`）+ `minimal` 档 + 组件实验室 `EditorShowcase` + 13 个单测用例。实测：1 万行首帧 **15.4ms**（热态）/ 123.2ms（dev 冷启动含模块解析），10 万行 38.1ms，两种规模均只挂 36 个 DOM 行；`vendor-editor` gzip **134.84KB**（修正 manualChunks 前为 572.36KB，语言包现按需分包）。

验收三条（用户原话「高性能、低占用、该有的功能都有」）：1 万行渲染 < 100ms；打包增量 ≤ 500KB（gzip）、单实例内存 < 10MB；22 项功能全部可用。**三条均达成**。

批 2 / 3 / 4 完成内容（2026-09-11）：

- **批 2**：中文查找替换面板（正则 / 全词 / 大小写 / 全部替换 / n-of-total 计数）、跳转行、快捷键（Ctrl+F/H/S/G、注释、移动复制行、Ctrl+D）、状态栏（行列 / 选中 / 语言 / 缩进 / 规模 / 编码）、格式化（JSON / XML / SQL）、语法校验波浪线（JSON / XML / SQL）、补全（注入源 + 文档词法兜底）、大文件两级降级（512KB 关高亮折叠补全、5MB 强制只读）、右键菜单事件、未保存标记。新增 `core/format/`（JSON / XML / SQL 纯函数下沉，编辑器与格式化工具共用）。
- **批 3**：删除自研 `LineNumberTextarea.vue`（6 处调用）与 `CodeViewer.vue`（3 处调用）以及数据库自研主题文件 `sqlEditorThemes.ts`；6 处文本域改 `UiCodeEditor mode="minimal"`，`SqlEditor.vue` 与 `EditorDialog.vue` 改写为薄封装（合计删掉约 880 行自研编辑器代码）。仓库内已无第二套编辑器实现。
- **批 4**：新增 `UiCodeDiff.vue`（`@codemirror/merge`，split / unified 两形态 + 增删行数统计）；SSH 保存冲突时后端附带远端内容，弹窗可「查看差异」对照远端与本地；`docs/03` 新增 §7「编辑器与文本输入规范」（禁止自研编辑器 / 复制主题，领域能力走注入口），`DESIGN.md` 补 `--cm-*` 说明。

实测：1 万行首帧 15.4ms（热态）；10 万行 38.1ms；`vendor-editor` gzip **135.62KB**（预算 ≤500KB）；空实例内存 **224KB**（预算 <10MB）；`pnpm lint / test / build` 全绿（29 文件 / 237 用例），Rust `cargo check` + 规范脚本通过。

后续补充（2026-09-12）：三轮视觉打磨（选区色统一为 `#79b8ff` / 暗色 `#264F78`）+ **编辑器统一契约固化为测试** `editorUnification.test.ts`（禁止第三方基座、禁止工具层直引 `core/ui/editor/*`、禁止残留 `LineNumberTextarea`/`CodeViewer`）+ 基座对比文档 `docs/tasks/2026-09-12-编辑器基座对比-CodeMirror6-vs-Monaco.md`（维持 CM6 结论）。

遗留：SSH 冲突差异未做真实服务器联调（需真实并发写入场景），首次联调建议在测试服务器临时文件上手工制造 mtime 变化。

---

## 二、✅ 已完成 · 框架存储目录配置（2026-09-12）

任务书：`docs/tasks/2026-09-11-存储目录配置任务书.md`

用户 2026-09-11 提出：希望有一个公共配置，统一管理缓存 / 日志 / 数据目录，可人工修改与迁移。现状是 8 处硬编码 `app_data_dir()`，无配置、无 UI、无迁移。

四个待定项已在 2026-09-12 按推荐值定案（用户授权本轮全部执行）：**本轮实施** / **`data` `vault` `logs` `cache` 四分区** / **重启生效**（不做运行中热切换）/ **迁移后保留旧目录**，由用户确认后手工清理。

| #   | 内容                                                                                                    | 状态       |
| --- | ------------------------------------------------------------------------------------------------------- | ---------- |
| 1   | `framework/paths.rs`：`storage_root` 统一入口 + 四分区解析 + 老布局一次性迁移                           | **已完成** |
| 2   | 7 处落盘点改走统一入口（插件数据库、vault、凭证、known_hosts、数据库 agent 驱动、数据库凭据、TTS 缓存） | **已完成** |
| 3   | `framework/storage/`：`storage_info` + `storage_migrate`（预检 → 复制 → 校验 → 写配置）                 | **已完成** |
| 4   | 设置页「存储位置」卡片（占用展示 / 修改位置 / 恢复默认 / 打开目录 / 迁移进度 / 重启引导）               | **已完成** |

- **老布局自动迁移**：启动时（`lib.rs` setup 最先，早于任何插件打开数据库）把根下 `*.db`、`vault.dat`、`credentials/`、`ssh-known-hosts`、`tts/`、`agents/` 搬入四分区；同卷 `rename` 优先，跨卷退化为复制 + 体积校验 + 删源；目标已存在则跳过绝不覆盖；记 `layoutVersion` 防重放。**迁移失败时路径解析回落旧位置**，升级不会丢数据。
- **位置迁移**：校验文件数与字节数逐分区比对、每个 SQLite 库 `PRAGMA quick_check`、`vault.dat` 长度语义，**全部通过才写配置**；只复制不删除源目录。
- 配置类根下文件（`settings.json` / `patchybox.json` / `.window-state.json`）**永不搬移**，故配置读取位置与数据放在哪个盘无关（自举安全）。
- 新增 11 个单测（路径解析、作用名合法性、复制统计、校验失败三分支、迁移幂等）。

---

## 三、✅ 已完成 · SSH 工具扩展四项（2026-09-12）

任务书：`docs/plugins/ssh/2026-09-11-扩展任务书.md`

| #   | 待办                | 状态       | 实现要点                                                                                                                                                                       |
| --- | ------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | 终端搜索（Ctrl+F）  | **已完成** | `@xterm/addon-search`；搜索栏组件 + `useTerminalSearch`                                                                                                                        |
| 2   | 右键「复制当前行」  | **已完成** | 按右击那一行的缓冲区取文本，折行向上合并                                                                                                                                       |
| 3   | 系统信息 + 磁盘明细 | **已完成** | Rust `ssh/system_info.rs` 采集（`LC_ALL=C` + `df -hlPT` 解析为纯函数并带单测），前端监控面板 30 秒刷新                                                                         |
| 4   | 会话日志            | **已完成** | Rust `ssh/log.rs`：终端通道 `ChannelMsg::Data` 分支旁路写盘，剥 ANSI + 敏感值打码；落 `<存储根>/logs/ssh/`（依赖上方存储配置，已就绪）；按终端一个文件，停止时回报路径与字节数 |

---

## 四、✅ 已完成 · FRP 客户端配置管理工具（2026-09-12）

调研：`docs/plugins/frp/2026-09-12-调研与可行性.md`（官方能力面 + ≥17 个同类开源项目 + 仓库可行性）
任务书：`docs/plugins/frp/2026-09-12-任务书.md`（契约冻结：17 条 IPC / 数据表 / 设置项 / 状态机 / 测试矩阵）

形态四个决策（2026-09-12 用户拍板，全选推荐项）：

- **frpc 二进制**：用户自备路径 + 工具内一键下载并校验（不做随包内嵌 sidecar）
- **后台常驻 / 开机自启**：v1 不做，只在打开工具时管理
- **配置存储**：直接管理本机多个真实 `frpc.toml`（与手写配置双向互通）
- **frpc webServer**：v1 不开，只用启停与日志

| #   | 批次 | 内容                                                                                 | 状态       |
| --- | ---- | ------------------------------------------------------------------------------------ | ---------- |
| 1   | 批 1 | Rust 后端：`models` / `profile` / `verify` / `runtime` / `binary` / `mod`（17 命令） | **已完成** |
| 2   | 批 2 | 前端契约与工作台：`contracts` / `ipc` / 档案列表 / 详情页签                          | **已完成** |
| 3   | 批 3 | 双模式编辑（表单 ⇄ 源码）+ 校验错误行 + 启停状态机 + 日志面板                        | **已完成** |
| 4   | 批 4 | frpc 引导（一键下载 / 指定路径）+ 设置项 + 文案 + 文档                               | **已完成** |

关键设计：

- **未知字段零丢失**：表单模式以原 TOML 解析结果（parsed）为基底做字段级覆盖，`healthCheck`、`metadatas`、自定义段落原样保留；含注释的文件在表单保存前提示注释会丢失（并自动备份 `.bak`）。
- **状态机在 Rust 侧**：日志行驱动 `stopped → starting → running/error`，带 30 秒兜底超时与进程退出码判定，避免「进程活着但早已连不上」的假绿灯；前端事件驱动 + 5 秒轮询兜底。
- **日志脱敏**：推送到前端的日志行剥 ANSI 并对 `token` / `password` / `secret` 打码。
- **删除走 `.trash/`**：不物理抹除。
- **零新增依赖**：下载与 SHA256 用已有 `reqwest` / `sha2` / `hex`；解压复用系统工具（Windows `Expand-Archive`、其它平台 `tar`）。

---

## 五、待办 · 框架可靠性与扩展治理

任务书：`docs/tasks/2026-09-12-框架可靠性与扩展治理任务书.md`

- [x] 2026-09-12：完成现有框架静态审查与详细任务书（23 条发现、13 项可靠性必修任务）；用户补充后增加1项必做本地数据功能，保留2项可选扩展。供后续 agent 执行，尚未实施代码整改或完成运行时验证。
- [ ] A 批：T04 / T05，系统密钥库真实接入、旧密钥兼容与凭证崩溃恢复。
- [ ] B 批：T01 / T02 / T03 / T06，存储生效根、离线迁移、旧布局恢复与事务迁移。
- [ ] C 批：T07 / T08 / T09，设置一致性、双平台交付与凭证引用。
- [ ] D 批：T10 / T11 / T12 / T13，工作区生命周期、诊断、IPC 契约与结构治理。

- [x] T15 专项任务书：`docs/tasks/2026-09-12-本地数据导入导出与云同步预留任务书.md`，已明确分类选择、数据空间隔离、融合冲突、凭证引用与故障恢复方案。
- [ ] E 批（近期必做）：T15 / L0–L5，本地数据导入导出、隔离/合并与云同步数据接口预留；先做 L0 冻结路径/设置边界，实施按 A–D 前置依赖推进。

用户 2026-09-12 确认：云账户为后期规划，用于多地登录同步配置、凭证、使用习惯；当前不做登录/云服务/实际同步，但必须预留稳定身份、空间与账户分离、传输适配和密钥边界。本期先实现本地数据功能。

P2 扩展 T14 / T16（工具意图、资源预算）仍仅为建议。后续执行前先核对任务书分析基线与当前代码，避免重复修复。

### 架构与规范独立评审（2026-09-12）

- [x] 完成 `docs/tasks/2026-09-12-架构与工程规范独立评审.md`：评审架构与 AGENTS 规则本身，包含8项架构判断、规范逐项修订建议、检查器探针和替换条文草案。
- [x] 确定调整方案并完成独立任务书：`docs/tasks/2026-09-12-架构重构与工程规范调整任务书.md`。评审建议已收敛为 AR01–AR07，尚未修改生效规范或实施代码重构。
- [x] AR01 规范切换（2026-09-12）：AGENTS / docs/01 / docs/02 / docs/03 / docs/05 / docs/README 与旧任务书冲突条文按任务书 §4.1–§4.2 收敛——owner 边界、规模改评审信号、composable 与注释按语义、锁与 clone 按成本、验证按风险分层；docs/02 拆分现行架构与设计期留档。
- [x] AR01 配套的两个工程 skill 同步（2026-09-12）：`patchybox-feature-flow` v1.2.0→v1.3.0（Rust 门禁与验证改按风险分层、本地只格式化本次文件、S3 不再要求四件全绿）、`patchybox-plugin-ui-conventions` v1.0.0→v1.1.0（规模红线改评审信号、新增 `core/ui` 不得依赖 stores/vault/业务 IPC）。源在 `C:\Users\patchy\AppData\Local\hermes\skills\software-development\`，该目录不受 git 管理：回退方式 = 按新版本号还原上述条目，生效时间以本条记录为准。
- [x] AR02 规范检查器重做（2026-09-12）：`src-tauri/tests/source_rules/`（syn 2 AST + 29 个夹具用例，入口 `scan_rust_rules` / `scan_docs`）取代逐行正则；旧实现的 2 处误报与 2 处漏检先用夹具复现旧行为、再验证新实现（测试模块之后的生产代码被整段截断、注释/字符串里的候选名误报、缩进 impl 方法漏检、`// SAFETY:` 与 `unsafe` 之间夹属性行误报）。两个 Python 入口改为薄 wrapper（`--exact` 过滤 + 必须实测运行 1 个测试才算通过）。基线改为「分类棘轮 + 14 条按路径/符号/类别/原因登记」的例外表。前端依赖守卫 `scripts/check_frontend_deps.mjs`（4 条规则 + 19 个夹具用例，8 条存量违规登记）。新增 dev 依赖 `syn =2.0.119` / `proc-macro2 =1.0.107`（对齐锁文件既有传递版本，未引入新包）。CI 显式跑 `--test source_rules` 与 `pnpm check:deps`（AR03 处理后须同步删除对应登记条目）。
- [x] AR02 遗留已随 AR03 解决（2026-09-12）：`EditorGoToLineBar.vue` / `EditorSearchBar.vue` 与 `ConfirmDialog.vue` / `InputDialog.vue` 改相对具体组件路径，不再经 `@/core/ui` 自身 barrel 回流；`scripts/frontend_deps_baseline.json` 的 8 条存量登记（R1×4 / R3×1 / R4×3，含编辑器两个 barrel 环）已全部清零。
- [x] AR03 基础 UI 与服务解耦（2026-09-12）：`core/ui/CredentialPicker.vue` → `core/vault/ui/`、`core/vault/CredentialForm.vue` → `core/vault/ui/`，新增 `core/vault/index.ts` 公开凭证复合组件与类型；`core/ui/useClipboard.ts` 拆为 `core/platform/clipboard.ts`（返回 ok/empty/failed，不提示）+ `core/feedback/useCopy.ts`（组合平台调用与 toast，三种反馈文案不变）；`core/ui/windowCtl.ts` → `core/platform/window.ts`（窗口操作收敛为 `runWindowAction`，界面不再持句柄）；消费方（dns 设置页、凭证管理页、SSH 服务器表单、8 个 useCopy 点、TitleBar）全部改新入口；`core/ui` 内部与凭证/编辑器组件改相对路径，R1/R3/R4 违规与环全部消失（守卫先跑一次输出 8 条「失效例外」= 旧问题已消除的取证，基线随即清零）。
- [ ] AR04 SSH 状态所有权重构（2026-09-12 目录归位已提交 `69f7c90`，**状态拆分未做**）：已完成 —— 60+ 平铺文件按 8 个能力域归位（profiles/connection/workspace/terminal/files/monitor/docker/tunnels），根目录只留入口与页面，仅路径与引用变化、零行为改动（pnpm lint/test/build/check:deps 全绿）。未完成 —— `useSshProfiles` / `useSshConnections` / `useSshLifecycle` / `useHostKeyQueue` 四个所有者模块的拆分与实际接入（含删除旧实现），以及任务书 §B3 前置要求的**行为特征测试先行**（先补连接/恢复/主机密钥/终端会话的特征测试，再拆状态）。开工前盘点已落盘于 `docs/plugins/ssh/2026-09-12-AR04状态拆分方案与行为清单.md`（提交 `b247dc9`）：同一职责现有三份实现（活跃根文件 723 行、无人消费的 `profiles/useSshProfiles.ts` 161 行与 `workspace/useSshIdleWatch.ts` 155 行），且孤儿实现缺根的迁移失败可见提示、`legacyCredentialsFailed` 分支、删除服务器连带关闭其连接工作区等较新逻辑；**待确认测试手法与执行者后开工**。
- [x] AR05 Database 前端与驱动职责重构（2026-09-12 **Rust 驱动与前端四域拆分均已提交**）：已完成 —— `drivers/mod.rs`（726 行）拆为 `session.rs`（会话注册表/取消句柄/快照）、`connection.rs`（建连/测试/断开）、`probe.rs`（版本与时延）、`execution.rs`（查询执行与取消分派），`mod.rs` 22 行只做声明与重导出（公开路径 `drivers::connect`/`drivers::DbState` 等保持不变，封闭枚举与 agent/dialect 边界未动）。前端已完成 —— `useDatabase.ts`（1416 行）按 §8.1 拆为 `connection/useDatabaseConnections.ts`(212 行)、`workspace/useQueryWorkspace.ts`(676)、`catalog/useDatabaseCatalog.ts`(702)、`library/useQueryLibrary.ts`(78) 与 175 行兼容门面（返回面 77 个 key 与拆分前逐一相等、14 处消费方 import 不变），提交 `5edcb4a`。未做 —— 域级行为特征测试：仓库前端测试目前只覆盖纯函数、无 `vi.mock` IPC 先例，需先确认为 composable 引入该范式（与 AR04 的 P3a 是同一个决策）。
- [ ] AR06 数据上下文与统一生命周期（2026-09-12 **机制部分落地，按任务书 §9.3 仍未完成**）：已落地 —— `framework/context.rs`（不可变 `DataContext{spaceId, generationId, location, epoch}`：四分区位置启动时解析一次并固定，`paths`/`PluginDb` 消费同一实例，不再每次调用现读 `settings.json`；`is_current`/`stale_dropped` 承担晚到事件判定与可诊断计数；`maintenance_guard()` 是根迁移/导入提交/空间激活/更新安装的唯一互斥）与 `framework/lifecycle.rs`（唯一关闭入口：`prepare_close` 可拒绝、`dispose` 受 5s 总超时约束且兜住钩子异常；`CloseReason = tab/exit/restart/update/space-switch`；`lib.rs` 的 `RunEvent::ExitRequested/Exit` 已改走该入口，frp 进程清理与 http_ws 会话断开已登记为模块钩子）。未完成（有外部前置）—— ① 默认空间身份/索引与空间-设备字段路由依赖导入导出 L0/L1（未交付），当前 `spaceId=default`/`generationId=1` 只是承载位，设置仍是单文件；② 关闭流程的「未保存则拒绝」消费方（前端页签 + 可靠性 T10/T11）未接入；③ key 与凭证上下文（T04/T05/T09）未接入；④ 双平台冷启动与旧数据夹具验证未做。仅机制与接口落地不算完成。
- [x] AR07 统一 HTTP owner 与静态装配（2026-09-12）：`plugins/api/{mod,models}.rs` → `plugins/http_ws/persistence/`（`api_*` 命令名、`storageKey: api` 与 `data/api.db` 均不变，模块描述里显式记录历史存储键）；新增 `framework/module_manifest.rs` 静态清单宏（`patchybox_module!` 一处声明生成 IPC 入库元数据 + handler + 模块描述，注册名取实现路径末段，兼容别名须显式 `as "名"`）与路由宏 `patchybox_routes!`（一行/模块生成路由分支、装配顺序与 `validate_routing()`），删除全部手写命令表、8 个手写 `invoke_handler` 与 owner 枚举；未纳路由的 owner 由 `unrouted_owner()` 明确报错（不再静默 false）。**顺带修复实测缺陷**：迁移中发现 11 条命令（`tts_synthesize`、`frp_profile_create`、`dbc_connection_save`、`ssh_connect`、`ssh_profile_import`、`ssh_terminal_log_start` 等）实现并进了 handler 但从未入库 → 前端调用一直 command not found，清单化后自动入库并纳入契约表（147 条已发布命令）。

- [ ] 剩余项逐项计划（用户 2026-09-12 要求「一项一项做」）：`docs/tasks/2026-09-12-架构重构剩余项执行计划.md`。已完成 —— P1 AR05 前端四域拆分（`5edcb4a`）、P2 文档漂移清理（`55af37e` 补测试 / `2b3c5cb` 文档）。待办 —— P3 AR04 状态拆分（三小步，**待确认测试手法与执行者**）、P4–P6 AR06 的三个外部前置（可靠性 A 批 T04/T05、D 批 T10/T11、E 批 L0/L1）、P7 AR06 收口（双平台冷启动 + 旧数据夹具）。待确认决策集中在 `docs/tasks/2026-09-12-架构重构剩余项待决策书.md`（D1–D6，未表态按各条默认动作执行）。

执行顺序与完成标准以独立调整任务书为准；不要再按评审中的候选路线分别创建实现。既有密钥/迁移风险修复可先行，不等目录整理。

---

## 六、明确不做（本轮，用户 2026-09-11 定）

**SSH**：快速命令片段面板、`~/.ssh/config` 导入与配置导出、密钥生成与公钥部署、远程↔远程直传、跳板机 ProxyJump 多跳、多服务器同步输入（MultiExec）、日志自动轮转与压缩。

**编辑器**：语言服务级智能提示（LSP / 类型推断）、富文本与 Markdown 所见即所得、协同编辑、代码片段管理器。

**FRP**（v1 边界，2026-09-12 定）：关闭界面后 frpc 后台常驻与开机自启、流量与连接数监控、运行时热改代理（`/api/store/proxies`）、定时连接、frps 服务端管理、开 `webServer`。

---

## 七、已搁置（用户说过「先不管」，勿擅自开始）

| 事项                                             | 背景                                                                                     |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| 下载默认目录 fallback 到 `C:/` 导致 `os error 5` | `FileManagerTab.vue` 的默认下载目录兜底；改法是换 Tauri `downloadDir()` + 把错误翻成人话 |
| 终端 ANSI 调色板 / 主题选择器                    | 用户 2026-09-11：现在颜色挺好，不需要                                                    |
| 远程终端 PS1 上色（改远端 bashrc）               | 同上，一并搁置                                                                           |
| 远程栏刷新按钮与本地栏对称                       | 曾问过用户，未表态                                                                       |
