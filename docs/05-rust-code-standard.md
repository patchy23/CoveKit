# patchyBox Rust 代码规范（草案 v1 · 2026-09-05 与用户对齐中）

> 目标：消灭「图方便」的 panic 与 clone。规则必须可检查、可执行，每条给出允许/禁止/替代写法。
> 现状基线（2026-09-05 盘点）：非测试代码 unwrap/expect/panic 38 处，clone 171 处。

## 1. Panic 控制（核心规则）

**默认禁止**在运行期可达路径（IPC 命令、后台任务、网络/IO/解析）使用：
`.unwrap()` / `.expect()` / `panic!()` / `unreachable!()` / `unwrap_unchecked` / 可能越界的直接索引。

**允许的三个例外**（必须在代码注释中体现依据）：

| 例外 | 场景 | 写法要求 |
|------|------|---------|
| 启动期 fail-fast | `app.run().expect(...)`、IPC 命令重复注册 panic | 编程错误，越早炸越好；信息写清排查方向 |
| 编译期可证的静态不变量 | `Hmac::new_from_slice`（接受任意长度密钥）等 | 用 `.expect("一句话说清为什么不可能失败")`，禁止裸 `.unwrap()` |
| 测试代码 | `#[cfg(test)]` 模块内 | 不限 |

**判定口诀**：「这个值失败的路径，用户能通过正常操作触发吗？」——能触发就是运行期路径，必须返回 `Err`。

**替代写法**：`ok_or("…")?` / `ok_or_else(|| format!(…))?` / `match` 分支处理。

**现状整改清单**（按优先级）：

1. `plugins/database/catalog.rs` 14 处 `expect("xx 方言存在")`——方言由用户配置的 db_type 驱动，运行期可达（2026-08 MySQL TABLE_ROWS panic 即为同类事故），全部改返回 `Err`
2. `plugins/api/mod.rs`、`plugins/dns/mod.rs` 6 处 `guard.as_ref().unwrap()`——改 `ok_or`
3. `plugins/database/secrets.rs`/`store.rs` `expect("已初始化")`——确认 init 顺序保证后保留 expect 并注释依据，否则改 Err

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

**现状基线（2026-09-05 盘点）：全库无 `unsafe` / `Box::leak` / `transmute`，`&'static str` 均为字面量与 OnceLock 锁的正当用法——本规则以预防为主，零容忍执行。**

**禁止**：

- `Box::leak` / `std::mem::forget` 伪造 `'static`（spawn 要 `'static` 就用 `Arc` 共享或 owned 数据）
- `unsafe` / `transmute`（本库目前为零，保持为零；确需引入必须在评审中单独说明理由）
- 把 `MutexGuard` 内部数据的引用传出锁作用域（编译器会拦，禁止用任何手段绕过）
- 为绕 borrow checker 给结构体乱挂生命周期参数（`struct Foo<'a> { r: &'a T }` 借用地狱）——该 owned 就 owned

**允许的 `'static`**：字符串字面量类型（`&'static str` 错误信息、方言 SQL 常量表）、`OnceLock` 全局单例。

**审查信号**：函数签名出现两个以上显式生命周期参数时，停下来想想是否真的需要借用。

## 4. 锁纪律（既定规则重申）

- `std::sync::MutexGuard` 不跨 `await`（非 Send）；取数用小作用域块
- State 用 `std::sync::Mutex`，持锁时间极短；不用 tokio Mutex（无跨 await 需求）
- 嵌套锁注意析构顺序（见 conn.rs get_sftp_session 的 `cached_slot` 写法）

## 5. 错误处理

- IPC 命令返回 `Result<T, String>`，错误文案面向用户、中文、含可操作建议
- 禁止 `catch_unwind` 兜底吞错；禁止空 `Err(_)` 丢弃错误上下文
- SQL/远程命令拼接：标识符引用 + 白名单（既定安全规则）

## 6. 注释与验证（既定规则重申）

- 文件头注释 + 结构体/函数/字段注释 + 关键逻辑行注释（`scripts/check_docs.py` 强制）
- 合入门槛：`cargo fmt` + `clippy --no-default-features --lib -- -D warnings` + `cargo test --no-default-features` + `check_docs.py` 全绿

## 7. 提交前检查（不做编译期强约束，2026-09-05 用户决策）

提交前运行：

```bash
python scripts/check_rust_rules.py
```

- panic 风险与 clone 统计采用**棘轮基线**（`scripts/rust_rules_baseline.json`）：违规总数只许减不许增；整改后同步下调基线数字
- 生命周期/unsafe 类（`Box::leak` / `unsafe` / `transmute` / `mem::forget`）**零容忍无基线**，出现即失败
- §1 的合法例外（启动 fail-fast、可证不变量）登记在脚本的 `ALLOWLIST_PATTERNS`，新增例外须先过评审再加白名单
