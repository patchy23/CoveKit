# 19 · Tauri 契约审计（五层命令矩阵与事件数据流）

> 本文原为 AI 助手侧技能库中的项目知识，2026-09-13 迁入仓库，作为项目自有开发手册的一部分。正文未改写（仅同步文档路径与目录规范）。

适用于插件阶段 B 的“只审查、不改代码”契约核对。目标不是只比对类型名，而是验证命令从声明到真实 UI 消费的完整数据流。

## 五层命令矩阵

对每个命令同时统计：

1. `contracts.ts` 命令常量、Payload、Result
2. 前端 `ipc.ts` wrapper 与真实 wire args（特别是 `{ payload: ... }` 包裹）
3. Rust IPC registry 文档登记
4. `tauri::generate_handler!` 注册
5. `#[tauri::command]` Rust 函数实现
6. 全部前端调用点数量（额外列，用来识别死命令）

矩阵应报告每层总数、差集、重复项和无调用命令。不要仅凭 registry 判断命令可调用。

## 必查契约维度

- 顶层参数包裹：TS 业务 payload 与 Tauri 顶层参数不是同一类型；为 wire args 单独建类型。
- camelCase：核对 Tauri command 参数规则和 serde `rename_all`，不要仅看 Rust 字段名。
- 可选值：missing、`undefined`、`null`、Rust `Option<T>`、serde 是否省略。
- 数字宽度：`u16/u32/u64` 对 TS `number`；输入范围、强制 cast 截断、`2^53-1` 精度。
- 返回语义：`Result::Err`、`ok=false+error`、成功时是否仍携带 error、远程 exit status 是否被吞。
- 事件：事件名、Rust emit payload、TS listener 泛型、取消订阅、真实 UI 消费者。
- 状态机：契约中的每个状态是否真的 emit；当前窗口的手工状态更新不能替代跨窗口事件。
- 语义值：字段类型一致仍可能长期返回 `0/false` 占位，或后端返回展示文本而前端按枚举比较。

## Tauri 注册关键坑

Tauri 2 的 `Builder::invoke_handler` 是 setter，不会累加 handler。多个插件各自调用时，后调用者覆盖前调用者。审计注册必须查看应用装配顺序，并在本地 cargo registry 的 `tauri/src/app.rs` 核实当前版本实现。最佳修复是全应用单次集中 `generate_handler!` 或显式组合 dispatch。

## 文件传输数据流检查

异步命令常只返回“已启动”，完成结果由事件推送。必须沿以下链路核对：

`invoke immediate result → transferId → Rust background emit → listener payload → component subscription → unlisten → UI completion/error`

特别检查：

- UI 是否错误地只检查立即返回的 `done=false`
- listener 是否声明了后端不存在的字段
- `write`/`flush` 错误是否被 `let _ =` 吞掉
- `transferred/total` 是否是真实字节数，而非人为 `+1`

## 推荐验证

- 自动脚本提取五层集合和调用点；对多行 chained 调用要二次符号搜索，避免正则错归属。
- `cargo test plugins::<id> --lib`
- `vue-tsc --noEmit`
- 审计前后运行 `git diff --exit-code -- <scope>` 和 `git status --short`，证明未修改文件。
- 报告每个问题必须包含：严重级、`文件:行`、可执行复现、修复建议。
