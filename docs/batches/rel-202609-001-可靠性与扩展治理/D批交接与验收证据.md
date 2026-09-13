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

- T10 验收里的真机走查本轮**没有证据**：页签快速开关 100 次后订阅/定时器回到基线、
  隐藏再显示后连接仍正常、退出时长任务可取消退性、macOS 上与系统快捷键冲突场景。
- T11 的真机走查同样未做：真实更新下载/安装（占位公钥环境下不可用）、诊断报告在真实
  WebView 里复制到剪贴板、错误清单在真实崩溃路径下的留档。
- 「关闭页签前提供保存入口」只实现了「取消 / 放弃并关闭」两个出路：保存动作属于各插件自己的
  编辑流程，框架不代它决定，需要产品确认是否要求插件在 `prepare` 里提供保存钩子。
- T13 剩余未做：`docs/02`、`docs/03`、`docs/05` 与 AGENTS 待办对齐；三条新架构守卫
  （绕过 `paths` 落盘、明文 secret 设置、框架引用业务表）；public UI 死代码复核；
  `framework/vault/store.rs` 458 行仍超 400 行。
