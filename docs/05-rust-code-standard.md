# patchyBox Rust 代码规范（v1.1 · 2026-09-05 定稿）

> 目标：消灭「图方便」的 panic、clone 与隐患写法。规则必须可检查、可执行，每条给出允许/禁止/替代写法。
> 现状基线（2026-09-05 盘点）：非测试代码 panic 风险 27 处，clone 170 处，`unwrap_or*` 141 处，`as` 数值转换 60 处，unsafe/Box::leak 为 0。

## 1. Panic 控制（核心规则）

**默认禁止**在运行期可达路径（IPC 命令、后台任务、网络/IO/解析）使用：
`.unwrap()` / `.expect()` / `panic!()` / `unreachable!()` / `unwrap_unchecked`。

**允许的三个例外**（必须在代码注释中体现依据）：

| 例外 | 场景 | 写法要求 |
|------|------|---------|
| 启动期 fail-fast | `app.run().expect(...)`、IPC 命令重复注册 panic | 编程错误，越早炸越好；信息写清排查方向 |
| 编译期可证的静态不变量 | `Hmac::new_from_slice`（接受任意长度密钥）等 | 用 `.expect("一句话说清为什么不可能失败")`，禁止裸 `.unwrap()` |
| 测试代码 | `#[cfg(test)]` 模块内 | 不限 |

**判定口诀**：「这个值失败的路径，用户能通过正常操作触发吗？」——能触发就是运行期路径，必须返回 `Err`。

**替代写法**：`ok_or("…")?` / `ok_or_else(|| format!(…))?` / `match` 分支处理。

**集合索引**：优先 `get()` / `first()`；已做长度检查的索引允许，但检查代码必须与索引相邻可见。

**`unwrap_or*` 家族（v1.1 新增）**：`unwrap_or` / `unwrap_or_default` / `unwrap_or_else` 是「静默兜底」，不经 panic 但会吞掉错误。允许用于**纯展示类字段**（文件列表的 owner/group 显示空串）；**关键数据禁止静默兜底**（金额、行数、尺寸、密钥、路径——错了必须显式报错或写 `0` 并注释「为什么 0 是安全语义」）。

**存量整改已完成（2026-09-05，基线 27 → 0）**：

1. ~~catalog.rs 14 处方言 expect~~ → 全部改走 `dialect_or_err()`（`dialect/mod.rs` 新增的 Result 版入口）
2. ~~api/dns 6 处 `guard.as_ref().unwrap()`~~ → `ok_or("本地库未初始化")?`
3. ~~secrets/store `expect("已初始化")`~~ → `ok_or_else`；catalog 3 处 `unreachable!` → 显式 Err
4. drivers 探测函数常量方言 → let-else 降级空版本（探测本来 best-effort）

## 2. Clone 纪律

**原则**：加 `.clone()` 前必须能回答「谁拥有这份数据？为什么不能用引用/Arc？」。为让 borrow checker 闭嘴而 clone = 所有权设计有问题，应重构。

**允许的 clone**：

| 场景 | 要求 |
|------|------|
| 跨 `await` / `tokio::spawn`（需 `'static`） | 句柄类一律 `Arc::clone`；值数据确需独立副本才 `.clone()` |
| 从 `Mutex` 取数后 await（锁不跨 await 既定规则） | 只取最小必要字段，不整体 clone 大结构 |
| IPC 出参 / 事件 payload | 要 serde 序列化给前端，owned 不可避免，允许 |

**禁止**：

- 热路径（每行/每条记录的循环体内）clone 大结构
- clone 后只读使用（应传引用）
- `to_string()` / `to_owned()` 只为绕过生命周期报错

**审查信号**：单个函数内 ≥3 个 clone → 停下来自查所有权设计。

## 3. 生命周期纪律

**现状基线：全库无 `unsafe` / `Box::leak` / `transmute`，`&'static str` 均为字面量与 OnceLock 锁的正当用法——本规则以预防为主，零容忍执行。**

**禁止**：

- `Box::leak` / `std::mem::forget` 伪造 `'static`（spawn 要 `'static` 就用 `Arc` 共享或 owned 数据）
- `unsafe` / `transmute`（本库目前为零，保持为零；确需引入必须在评审中单独说明理由）
- 把 `MutexGuard` 内部数据的引用传出锁作用域（编译器会拦，禁止用任何手段绕过）
- 为绕 borrow checker 给结构体乱挂生命周期参数（`struct Foo<'a> { r: &'a T }` 借用地狱）——该 owned 就 owned

**允许的 `'static`**：字符串字面量类型（`&'static str` 错误信息、方言 SQL 常量表）、`OnceLock` 全局单例。

**审查信号**：函数签名出现两个以上显式生命周期参数时，停下来想想是否真的需要借用。

## 4. 锁与异步纪律

- `std::sync::MutexGuard` 不跨 `await`（非 Send）；取数用小作用域块
- State 用 `std::sync::Mutex`，持锁时间极短；不用 tokio Mutex（无跨 await 需求）
- 嵌套锁注意析构顺序（见 conn.rs get_sftp_session 的 `cached_slot` 写法）
- **async 上下文禁止同步阻塞调用**（v1.1 新增）：`std::fs::*` / `std::thread::sleep` / 阻塞网络库会卡住 tokio worker（全库共 86 处 `std::fs` 在 async 可达路径，多为小文件读写，存量可暂留）。新代码用 `tokio::fs` 或 `tokio::task::spawn_blocking`；存量触碰随手迁移
- **`tokio::spawn` 任务必须有归属**：谁创建谁负责取消/回收（参照 ssh 终端的 cancel 通道模式）；禁止「发了就忘」的游离任务（长驻监听任务除外，需注释说明生命周期）

## 5. 错误处理

- IPC 命令返回 `Result<T, String>`，错误文案面向用户、中文、含可操作建议
- 禁止 `catch_unwind` 兜底吞错；禁止空 `Err(_)` 丢弃错误上下文
- **错误信息不得携带敏感数据**（v1.1 新增）：凭证、密钥、token、含密码的连接串不进错误文案与日志；上游错误（如 russh 认证失败原文）透传前先确认不含入参回显
- SQL/远程命令拼接：标识符引用 + 白名单（既定安全规则）

## 6. 数值与类型转换（v1.1 新增）

- 优先 `try_from()` / `try_into()` 并处理错误；`as` 强转仅限**范围可证安全**的场景，并在注释写明依据
- 典型事故面：有符号 → 无符号（负数回绕成巨值，如 `mtime as u64`）、u64 → u32/usize（截断）、f64 → 整数（精度与符号）
- 解析外部输入（用户输入、远程数据、文件内容）的数值必须带错误处理，禁止 `parse().unwrap_or(0)` 式静默归零（除非 0 是明确的安全语义）

## 7. 日志约定（v1.1 新增）

- 一律 `eprintln!` 并带 `[模块]` 前缀（如 `[vault]` `[ssh]`），便于在 dev 控制台过滤
- 禁止 `println!`（会进 stdout 污染打包产物）；禁止日志打印敏感信息（同 §5）
- 高频路径（轮询/逐行输出）不打日志，或打降级后的摘要

## 8. 依赖引入评审（v1.1 新增）

新加 crate 前回答四个问题（写进提交说明）：

1. 维护状态：近一年有提交？issue 响应正常？
2. 许可证：MIT/Apache-2.0/BSD 兼容？
3. 传递依赖膨胀：拉进来的依赖树多大？
4. 平台绑定：是否单平台？（本项目铁律：禁单平台方案）

优先复用已引入生态（tokio / serde / russh / reqwest / reka），能小步组合就不引新包。

## 9. 注释与验证（既定规则重申）

- 文件头注释 + 结构体/函数/字段注释 + 关键逻辑行注释（`scripts/check_docs.py` 强制）
- 合入门槛：`cargo fmt` + `clippy --no-default-features --lib -- -D warnings` + `cargo test --no-default-features` + `check_docs.py` 全绿

## 10. 提交前检查（不做编译期强约束，2026-09-05 用户决策）

提交前运行：

```bash
python scripts/check_rust_rules.py
```

- panic 风险与 clone 统计采用**棘轮基线**（`scripts/rust_rules_baseline.json`）：违规总数只许减不许增；整改后同步下调基线数字
- 生命周期/unsafe 类（`Box::leak` / `unsafe` / `transmute` / `mem::forget`）**零容忍无基线**，出现即失败
- §1 的合法例外（启动 fail-fast、可证不变量）登记在脚本的 `ALLOWLIST_PATTERNS`，新增例外须先过评审再加白名单

## 附录 · 参考来源（2026-09-05 对照验证）

本规范与以下社区公认标杆交叉验证过：

| 来源 | 星数 | 与本规范的关系 |
|------|------|---------------|
| [rust-unofficial/patterns](https://github.com/rust-unofficial/patterns) | 8.9k★ | 官方反模式第一条「Clone to satisfy the borrow checker」= 本规范 §2 的原始出处；「`#[deny(warnings)]` 是反模式」印证我们用 CLI 的 `-D warnings` 而非代码内 deny 的做法正确 |
| [rust-lang/api-guidelines](https://github.com/rust-lang/api-guidelines) | 1.3k★ | 官方 API 清单；本规范 §1 比官方更严（官方允许库代码 unwrap，我们因 IPC 直连用户而收紧），方向一致 |
| [pretzelhammer/rust-blog](https://github.com/pretzelhammer/rust-blog) | 8.4k★ | 《Common Rust Lifetime Misconceptions》= §3 的理论依据 |
| [google/comprehensive-rust](https://github.com/google/comprehensive-rust) | 33k★ | Google Android 团队课程，错误处理章节与 §5 一致 |
| [rust-lang/rust-analyzer](https://github.com/rust-lang/rust-analyzer) | 16.8k★ | 其 dev 风格指南的锁与异步纪律 = §4 出处 |
