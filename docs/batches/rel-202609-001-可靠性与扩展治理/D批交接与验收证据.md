# D 批交接与验收证据（rel-202609-001 可靠性与扩展治理）

> 范围：D 批计划为 T10 → T11 → T12 残余 → T13。
> **本轮实际完成：T12 残余、T13（部分）。T10、T11 未开工。**
> 未开工原因：这两项是行为密集的工作区生命周期改造（关闭协商入口、退出协调、资源释放、
> 统一长任务观察与诊断），需要在真实应用里反复走查（页签快速开关、隐藏/恢复、退出时长任务可取消），
> 无人值守时段无法取得可复核的真机证据，因此没有开工，避免留下半成品架构。

## T12 残余 · 注册表与 IPC 边界可验证（已完成）

- 工具注册表重复 id、非法分类、工具内设置键重复、select 缺选项一律**立刻抛错**并指出冲突双方，
  不再 `console.warn` 后覆盖（覆盖会静默少一个工具或一组设置）。
- 注册表改为可实例化（`createToolRegistry`），默认实例供应用使用；测试与嵌入式场景互不污染。
- 新增命令级一致性校验 `validate_command_declarations`：模块清单声明的命令与注册表登记的命令
  **逐条**比对，少登记或漏声明都在启动期失败；此前只比对归属者名字，命令少一条要等到运行期 404。
- 校验已接入启动路径（`lib.rs`），并有负例用例证明「只差一条命令」也会失败。
- 关于锚点里的 `window` / `open_external` / `framework_commands`：当前实现中三者都在框架命令表里声明
  且已登记，新校验通过即为证据；真出现漏登记时新校验会立刻失败。

## T13 · 去除事实源冗余与框架结构治理（部分完成）

已完成：

- `framework/storage/layout.rs`（921 行）拆为四个文件：编排（`layout.rs` 371 行）、集合矩阵
  （`layout/groups.rs` 185 行）、结果模型（`layout/report.rs` 125 行）、搬运引擎（`layout/engine.rs` 336 行），
  迁移用例随实现归位并纳入同一套行数约束。
- `framework/secure_store/key.rs`（640 行）拆为声明层（`key/mod.rs` 396 行）与解析层（`key/resolve.rs` 256 行），
  解析入口保持原路径导出，调用方零改动。
- 拆分后仍通过文档覆盖规则：迁移矩阵的常量补齐职责注释，新增文件都有 `//!` 模块职责说明。
- T13-2（加密原语重复）与 T13-3 的 AppSettings / 镜像冗余已由 T05、T07 完成。

未完成（需要后续会话）：

- T13-4 文档同步：`docs/02` 当前架构、`docs/03` 目录与数据库规则、`docs/05` 平台例外、
  AGENTS 待办与 TODO 与现状对齐（本轮只更新了台账与 TODO，未通读三份规范文档）。
- T13-5 新增架构守卫（禁止绕过 paths 取业务落盘根、禁止新建明文 secret 设置、
  框架层禁止直接引用业务表）：现有层级守卫（framework 引用 plugins、插件互相 import）保持有效，
  三条新守卫未实现。注意框架引用业务表的守卫在 T09 完成后已有事实基础（框架里已无插件 SQL）。
- T13-6 public UI 死代码复核未做。

## 验证

- Rust：`cargo fmt --check`、`cargo clippy --all-targets -D warnings`、`cargo test --no-default-features` 通过；
  lib 285 项通过 / 3 忽略，`source_rules`（代码规则 + 文档规则）29 项通过。
- 新增用例：命令级一致性正例与负例各 1 项；工具注册表 6 项（重复 id、非法分类、设置键重复、
  select 缺选项、实例隔离、分类计数）。
- 前端：`pnpm run lint`、`pnpm run test`、`pnpm run build`、`pnpm run format:check` 通过。
- 脚本：`check_rust_rules.py`、`check_docs.py`、`check_progress.py`、`check_markdown.py`、`check_doc_budget.py` 通过。

## 拆分后的文件规模

| 文件 | 行数 |
| --- | --- |
| `framework/storage/layout.rs` | 371 |
| `framework/storage/layout/engine.rs` | 336 |
| `framework/storage/layout/groups.rs` | 185 |
| `framework/storage/layout/report.rs` | 125 |
| `framework/secure_store/key/mod.rs` | 396 |
| `framework/secure_store/key/resolve.rs` | 256 |
| `framework/paths.rs` | 324（布局迁移与旧 helper 移出后） |
| `framework/vault/store.rs` | 458（删除框架侧插件 SQL 后的现状，仍超 400，见下） |

## 未验证项与限制

- 未开工项：T10、T11 全部子项（含验收里的「快速开关 100 次订阅回到基线」「退出时长任务可取消退性」
  「隐藏再显示连接仍正常」「单工具抛错其它工具可用」）都需要真机走查，本轮无证据，因此不勾选。
- `framework/vault/store.rs` 仍超过 400 行（458 行）：拆分需要同时处理 `read_all_at` / `write_all_at` /
  保护状态与凭据模型多处调用，留待 T13 后续会话一并处理，不用豁免掩盖。
- 守卫只覆盖现有两条层级规则；新增三条守卫未实现，故意注入违规的验证因此也没有证据。
