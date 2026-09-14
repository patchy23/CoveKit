# D 批交接与验收证据（rel-202609-001 可靠性与扩展治理）

> 范围：D 批为 T10 → T11 → T12 → T13。
> 首轮完成了 T12 与 T13 主体；**T10、T11 在第二轮补齐**（见下方两节），
> T13 剩余项（文档同步、三条架构守卫、public UI 死代码复核）仍未完成。

## T10 · 工具生命周期（已完成）

### 关闭协商（T10-1、T10-2）

- 新增 `src/core/lifecycle/toolContext.ts`：插件以 owner 身份登记「怎么关」——`prepare` 返回
  拒绝理由（未保存内容、任务运行中）、`dispose` 负责释放资源；框架只做协调，不替插件决定。
- 关闭变成异步协商：**任一 owner 拒绝就不清理、不关闭**，界面把理由逐条列出，用户可取消或
  「放弃并关闭」。逐 owner 的清理失败单独收集并成条上报，不把「清到一半」当成功。
- 清理有总超时（默认 5 秒，`DEFAULT_DISPOSE_TIMEOUT_MS`）：超时按失败计入但仍完成关闭，不拖死界面。
- 关闭入口统一：页签关闭按钮、`Ctrl/Cmd+W`、溢出菜单全部走 `ui.requestClose`；
  低层 `closeTab`/`closeAllTabs` **不再从 store 导出**，绕过路径在类型层面拿不到（`vue-tsc` 把关）。
- 页签上以圆点标记「未保存 / 运行中」，来自 `watchToolCloseState` 的订阅，不是各页面自己猜。

### 退出协调（T10-3）

- 新增 `framework/exit.rs`：`app_request_exit`（业务可拒绝）与 `app_force_exit`（用户显式强退，
  跳过拦截但仍在总超时约束内），前端 `src/core/lifecycle/appClose.ts` 是对应唯一入口。
- 被拒绝时后端把主窗口唤到前台并推送 `app://close-vetoed`，界面弹窗给出重试 / 强制退出 / 取消；
  托盘退出的 `ExitRequested` 复用同一套提示与强制标记，不再只写一行日志。
- 事件在窗口隐藏时不带标记的强制退出仍会先跑 dispose 钩子，超时按失败记录。

### 插件资源接入（T10-4）

| 插件 | 关闭工具页签时 | 有活动资源时 |
| --- | --- | --- |
| SSH | 断开全部会话并停止隧道 | 先询问（列出连接数） |
| FRP | 停止本工具启动的 frpc 进程 | 先询问（列出运行中档案数） |
| 数据库 | 断开全部连接（连接池随页签释放） | 不询问（无数据丢失风险） |
| 接口调试 | 关闭已打开的 WebSocket 会话 | 不询问；HTTP 请求后端一次性执行，不做假清理 |

策略写在各插件 `toolLifecycle.ts` 的模块注释里，行为由 4 个测试文件锁住（15 项）。

### 作用域订阅与错误边界（T10-5、T10-6）

- `src/core/lifecycle/scope.ts`：订阅与定时器集中在作用域里释放；**卸载早于 `listen` resolve 时
  resolve 后立即解绑**；建订阅部分失败时清理已建部分；dispose 之后拒绝再建定时器。
- `src/features/workspace/ToolHost.vue`：单个工具页签的宿主，加载失败与渲染异常都收敛在页签内，
  可单独重试（重建组件实例），不影响其它页签；工具是异步组件，重试必须换新实例才生效。

### 隐藏降频（T10-7）

- `toolVisibility` 区分**激活 / 被设置页覆盖 / 窗口隐藏**三种来源，切换页签不再等于断开连接。
- FRP 轮询、SSH 监控、接口调试的 WS 轮询接 `throttledInterval`：页签切走或窗口隐藏后降频，
  恢复可见时立刻取一次最新状态（`scope.onResume`），隐藏期间不停止协议心跳本身。

## T11 · 长任务与诊断（已完成）

### 长任务登记（T11-1）

- 新增 `framework/tasks.rs`：任务带 id / 归属 / 类型 / 状态 / 进度 / 失败错误码 / 不可取消原因；
  活跃任务上限 8、已完成历史 20 条，超出拒绝登记并明确提示。进度不可估算时保持为空，不填 0。
- 存储迁移在启动维护阶段登记为任务并上报进度，成功与失败都进最近结果；无待执行计划时不登记，
  避免每次启动留一条「无事发生」的记录。
- 取消口径如实声明：框架长任务当前**都不可中途取消**（迁移中断会留下半成品，下次启动续跑），
  因此没有伪造 `pending`/`cancelled` 状态，也不提供取消命令；需要取消的阶段由前端自己管（见下）。

### 更新状态脱离设置页（T11-2）

- `src/stores/update.ts`：检查 / 下载 / 安装状态、进度、待安装版本、失败原因都在 store 里，
  离开设置页再回来仍可见；组件不再持有更新句柄的局部状态。
- 阶段口径明确：`downloading` 与 `ready` 可取消（关闭句柄并丢弃已下载内容），
  `installing` 不可取消并说明中断后果；各阶段重入保护，连点不会起第二次检查/下载。

### 错误留档与诊断（T11-3、T11-4、T11-5、T11-6）

- `src/core/diagnostics/errors-core.ts`：有界清单（50 条上限、说明 300 字符、明细 600 字符），
  `code + message` 相同则合并计数；`errors.ts` 装 Vue 错误钩子、未处理 Promise 拒绝与全局脚本错误。
- `src/core/diagnostics/report.ts`：诊断报告只含白名单字段（版本、Tauri 版本、平台、存储根是否默认、
  保护模式、活跃任务数、最近任务、错误码），路径统一脱敏为 `<path>`，**只在本机生成、不做任何上传**。
- 更新失败带稳定错误码（`update.check_failed` / `update.download_failed` / `update.install_failed`），
  并进入错误清单；设置页新增诊断卡片可复制报告与清空记录。
- 渐进迁移口径：插件 IPC 的字符串错误**本轮未改**（未统一成 `{code,message,requestId}` 信封），
  框架侧只保证新增路径带稳定 code，避免一次性改动全部插件契约。

## 验证（HEAD `b7f02fb`）

- Rust：`cargo fmt --all -- --check`、`cargo clippy --no-default-features --all-targets -- -D warnings`、
  `cargo test --no-default-features` 全绿；lib **296 项通过 / 3 忽略**（此前 285），
  `source_rules` **29 项通过**。
- 前端：`pnpm run build`（含 `vue-tsc`）、`pnpm run lint`、`pnpm run format:check` 全绿；
  `pnpm run test` **478 项通过 / 54 文件**（此前 419 / 43）。
- 脚本：`check_rust_rules.py`、`check_docs.py`、`check_progress.py`、`check_markdown.py`、
  `check_doc_budget.py`、`test_doc_checks.py`、`check_versions.py` 全绿。
- `check_release_config.py` 属发布环境检查：本机无 `TAURI_UPDATER_PUBLIC_KEY` 时按设计失败，
  用临时密钥生成配置后校验通过（临时配置已删除，未留在工作区）。

## 未验证项与限制（必须真机复核）

- T10 的真机走查于 2026-09-14 在正式版上做了一轮（见下节）：关闭走托盘、网格与设置页状态已取证；
  页签快速开关 100 次后的订阅/定时器计数、隐藏再显示后连接仍正常、退出时长任务可取消三项
  **仍未取证**，原因见下节末段。
- T11 的真机走查于 2026-09-14 部分取证：诊断报告已在真实 WebView 复制到剪贴板（并据此修掉
  权限提示缺陷）；真实更新下载/安装（占位公钥环境下不可用）、错误清单在真实崩溃路径下的留档未取证。
- 「关闭页签前提供保存入口」只实现了「取消 / 放弃并关闭」两个出路：保存动作属于各插件自己的
  编辑流程，框架不代它决定，需要产品确认是否要求插件在 `prepare` 里提供保存钩子。
- T13 于第二轮收尾完成：文档同步、架构守卫、`vault/store.rs` 拆分与 public UI 死代码复核均见下节；
  仅剩「明文 secret 设置」未另加源码级守卫（由设置层运行期拒绝覆盖，见下节口径）。

## T10/T11 真机走查（2026-09-14，正式版）

走查方法：`pnpm tauri build` 出当前 HEAD 的正式版，以 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=`
`--remote-debugging-port=9222` 启动，用 CDP `Runtime.evaluate` 按文本驱动真实页面并读回 DOM 文本；
窗口级状态用桌面驱动取窗口列表与截图交叉判断。原生弹层不进入 AX 树与窗口截图，只能靠页面文本与
外部状态（剪贴板）比对确认。

已验证：

- **关闭主窗口 = 最小化到托盘（设计行为，非缺陷）**：`lib.rs:73-81` 对 `CloseRequested` 走
  `prevent_close()` + `hide()`，实测主窗口消失、进程留在托盘，仅剩单实例隐藏窗口 `…-siw`。
- **正式版工具网格 8 个且无「组件实验室」**：网格文本为格式转换 / 接口调试 / 数据库 / Hosts 编辑 /
  DNS 解析 / SSH 远程管理 / FRP 客户端 / 文字转语音，与构建产物里 grep 不到该工具的结论一致。
- **设置页渲染真实状态**：存储位置默认目录 32.03 MB / 25 个文件、凭证库两个保护域徽章、
  更新「不可用：当前构建使用占位更新公钥」、诊断「进行中的长任务：0」「暂无错误记录」。
- **诊断报告复制到真实 WebView 剪贴板**：点击后 toast 显示「已复制到剪贴板」，外部读剪贴板得到
  8 行报告——应用版本 0.1.0 / Tauri 2.11.5 / 平台 Windows / 存储根默认目录 4 个分区 /
  系统密钥库保护已启用 / 保护域 `vault：system-keyring（available）`、`credentials：file-fallback（available）` /
  活跃任务数 0，且不含任何本机绝对路径（白名单与脱敏生效）。

走查发现并修复的两处缺陷，提交 `42d3f36`：

1. **快捷键「未生效」误报（每台机器必现）**：`settings.json` 中 `globalHotkey='Ctrl+Shift+Space'`，
   框架写入的 `globalHotkeyActive='shift+control+Space'` 是 Shortcut 规范形式，`stores/settings.ts`
   却按字面相等比较，于是界面常驻橙字「当前快捷键未生效（可能已被其他程序占用）…」，
   而启动日志里没有任何注册失败。改为规范化比较（大小写、修饰键别名与顺序），
   单测夹具改用框架真实形态，原先用人类形式正好把缺陷掩盖。
2. **复制诊断信息触发 WebView2 权限提示**：`DiagnosticsCard.vue` 直接调 `navigator.clipboard.writeText`，
   打包版首次复制会弹「http://tauri.localhost 想要查看复制到剪贴板的文本和图像 / 阻止 / 允许」，
   未授权前剪贴板内容不变。桌面容器统一改走 `@tauri-apps/plugin-clipboard-manager`
   （数据库 / SSH 既有做法），网页 API 只留浏览器预览回退，失败按 `ok:false` 给出「复制失败」文案。

仍未取证及原因：

- 页签快速开关 100 次后订阅/定时器回到基线：正式版没有页内计数器，外部不可观测；
  取数需要 dev-only 钩子或临时插桩构建，本轮未做。
- 隐藏再显示后连接仍正常：需要真实 SSH 目标与凭证，本机无可用凭证。
- 退出时长任务可取消：需要真实长任务夹具，避免在真实数据目录上制造迁移。
- macOS 快捷键冲突、真实更新下载与安装：非本机平台 / 占位公钥环境。



### 结构拆分

- `framework/vault/store.rs` 由 458 行拆为父文件 273 行 + `store/{api,io,paths,status,summary}.rs`
  （26/42/49/47/49 行），采用仓库既有 `foo.rs` + `foo/` 形式（同 `framework/storage/layout`），
  对外入口经重导出保持同名，`vault/mod.rs` 调用点未改。
- 主密钥解析与 AES-GCM 加解密仍只在 `secure_store`，本模块不重复实现；拆分只搬移布局、读写、
  保护状态与脱敏摘要四类职责。
- `framework/tasks.rs` 用例加串行守卫：登记表是进程级全局，并行跑会互相清空或占满上限，
  之前全量跑随机挂 2 条、单跑全过；改为每用例持有守卫后单跑与全量跑一致。

### 文档同步（对源码核对后修改，非按印象）

- `docs/standards/02-架构.md`：前端目录补 `core/lifecycle`、`core/diagnostics`；Rust 结构补
  `secure_store`、`credential_refs`、`exit`、`tasks`；依赖方向改为设置写入权威已收敛到 Rust；
  §3.1 存储布局改为现行行为（根不可用不静默回退默认目录、老布局迁移改为登记计划并在启动维护
  阶段执行+重启生效），并把 §3.1 明确划出「2026-08 设计留档」范围。
- `docs/standards/03-模块开发规则.md`：`sqlx` 改为原生驱动会话；`stronghold` 改为框架凭证库
  （系统密钥库主密钥 + AES-256-GCM）。
- `docs/standards/05-Rust代码规范.md`：`unsafe` 处数由「1 处」更正为 2 处（`win_acl.rs` ACL、
  `lib.rs` WebView2 菜单封禁）；`std::fs` 计数改为 2026-09-14 实测口径（395 处引用含测试，只作
  参考不作门禁）；日志一节如实标注 Rust 侧级别体系与轮转**尚未落地**，不再指向已完成的 T11。

### 架构守卫

- 前端（新增）：`src/core/registry/pluginBoundary.test.ts` 禁止业务插件互相 import，只允许
  `@/core/*`；当前真实树 0 违规，注入夹具能抓出违规。
- Rust 引擎（新增，零容忍）：`paths_bypass` 拦截绕过 `framework::paths` 直接取落盘根
  （`app_data_dir` / `app_config_dir` / `app_local_data_dir` / `app_cache_dir` / `app_log_dir`，
  `paths.rs` 为自举例外）；`foreign_table` 拦截 `framework` 下字符串字面量命中插件建表名，
  表名从插件 `CREATE TABLE` / `CREATE INDEX` 现场推导，不在守卫里硬编码。
- 两条 Rust 守卫统一取值范围：只扫首个 `#[cfg(test)]` 之前的生产前缀，并跳过 `*_tests.rs`
  测试专用文件；文本级近似与其边界写进 `COVERAGE_LIMITS`。
- 注入验证：把「`app_data_dir` 直取落盘根 + 字面量引用 `ssh_profiles`」注入
  `framework/ipc_registry.rs` 生产区后，`cargo test --test source_rules` **失败**并分别报出
  `paths_bypass`、`foreign_table`；还原后 32 用例全过（探针未留在工作区，已核对）。
- 「禁止新建明文 secret 设置」未另加源码级守卫：设置层已用 `SECRET_KEY_HINTS` +
  `suggests_secret()` 在写入前拒绝密钥类键，并有 `internal_and_secret_keys_are_refused`
  用例覆盖注入场景，再写一遍源码扫描只会更弱。2026-09-14 用户裁决：**不另加源码级守卫**
  （运行期拒绝已是唯一执行点，补一层文本扫描会更弱并多出第二份关键词清单）。

### public UI 死代码复核

- `core/ui` barrel 38 项导出全部有消费方；目录内 44 个组件无孤儿（`EditorGoToLineBar`、
  `EditorStatusBar` 由 `UiCodeEditor` 内部使用，非死代码）。
- `UiIcon`（通用线性图标）与 `features/ui/AppIcon`（应用/工具图标）职责不同、各有消费方，
  不属重复实现。
- `src/plugins/component-lab` 是注册在案的组件验收工具（category `dev`，无环境过滤，正式构建
  可见），文件头已写明用途；2026-09-14 用户裁决：**改为仅开发构建注册**（`import.meta.env.DEV`
  门控）：开发与单测照常注册，正式构建静态摇除，`pnpm build` 后 `dist` 已实测无该工具引用。

### 本轮验证（HEAD `b6b6d25`）

- Rust：`cargo fmt --all -- --check`、`cargo clippy --no-default-features --all-targets -- -D warnings`
  通过；`cargo test --no-default-features` lib **296 通过 / 3 忽略 / 0 失败**；
  `--test source_rules` **32 通过**（此前 29，新增 3 条夹具）。
- 前端：`pnpm run build`（含 `vue-tsc`）、`pnpm run lint`、`pnpm run format:check` 通过；
  `pnpm run test` **483 通过 / 55 文件**（新增插件边界守卫 5 项）。
- 脚本：`check_rust_rules.py`、`check_docs.py`、`check_progress.py`、`check_markdown.py`、
  `check_doc_budget.py`、`check_versions.py` 全绿；`check_release_config.py` 仍为发布环境项，
  本机按设计红。
- 本轮提交：`5bfa448`（凭证存储拆分）、`9016ffe`（长任务用例串行）、`aa92e2c`（文档同步）、
  `b6b6d25`（架构守卫）。

