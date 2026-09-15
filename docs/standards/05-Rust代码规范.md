# patchyBox Rust 代码规范（v1.1 · 2026-09-05 定稿）

> 目标：消灭「图方便」的 panic、clone 与隐患写法。规则必须可检查、可执行，每条给出允许/禁止/替代写法。
> 现状基线（2026-09-05 盘点）：非测试代码 panic 风险 27 处，clone 170 处，`unwrap_or*` 141 处，`as` 数值转换 60 处，unsafe/Box::leak 为 0。
> 更新（2026-09-11 用户决策）：`unsafe` 不再一律禁止——**必要且安全时允许使用，但必须带紧邻的 `// SAFETY:` 论证**（见 §3、§10）；`Box::leak` / `transmute` / `mem::forget` 仍零容忍。
> 更新（2026-09-12，AR01 规范切换）：`clone` 与 `unwrap_or*` 改为按语义与成本评审（不再以数量或方法名判定）；锁按是否跨 `await` 选型（不按类型名）；注释按语义强制；日志区分协议 stdout 与诊断通道；依赖按安全维护、兼容与总成本审查。panic 与生命周期零容忍底线不变。

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

**`unwrap_or*` 家族（2026-09-12 修订）**：判定标准是**缺省语义**而不是方法名。允许业务已明确定义的默认值（展示字段空串、可选项默认关闭、探测失败降级零值并注释依据）；**禁止把 IO / 解析 / 密钥 / 路径的失败伪装成成功的默认值**，这类失败必须返回 `Err` 或走显式错误分支。审查看 `Result` 如何处理、缺省是否可解释，不统计出现次数。

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
- clone 后仅作只读使用（通常应传引用）
- `to_string()` / `to_owned()` 只为绕过生命周期报错

**评审方式（2026-09-12 修订）**：按**数据规模、调用频率、所有权与是否持锁**判断，clone 总数不作为门禁或基线（脚本只报告，见 §10）。同一函数内出现多个 clone 是需要解释的信号，但小值快照与短生命周期副本可以 clone；`Arc` 有原子计数与共享复杂度，**不是默认更优解**，只在确实需要共享所有权时使用。

## 3. 生命周期纪律

**现状：`Box::leak` / `transmute` / `mem::forget` 全库为零，零容忍执行；`unsafe` 仅在「无安全替代的系统 API」场景允许（当前 2 处平台例外：`framework/secure_store/win_acl.rs` 的 Windows ACL 读写与 `lib.rs` 的 WebView2 文档/右键菜单封禁），且必须带 `// SAFETY:` 论证（2026-09-11 用户决策，2026-09-14 复核处数为 2）。**

**禁止**：

- `Box::leak` / `std::mem::forget` 伪造 `'static`（spawn 要 `'static` 就用 `Arc` 共享或 owned 数据）
- `transmute`（类型欺骗，一律用安全转换替代）
- 把 `MutexGuard` 内部数据的引用传出锁作用域（编译器会拦，禁止用任何手段绕过）
- 为绕 borrow checker 给结构体乱挂生命周期参数（`struct Foo<'a> { r: &'a T }` 借用地狱）——该 owned 就 owned

**`unsafe` 允许条件（四问，缺一不可，2026-09-11 用户决策）**：

1. **必要性**：确无安全替代（典型：Win32 / COM 等系统 API 只提供 unsafe 入口）
2. **安全性论证**：块上方必须有紧邻的 `// SAFETY:` 注释，写清满足的不变量（指针来源与生命周期、是否跨线程或跨 await、返回值如何处理）
3. **最小作用域**：`unsafe` 只包住必须的那几条调用；禁止整函数包裹，禁止用它绕借用检查或省 clone
4. **无 UB 路径**：不允许「大概率没事」的裸指针解引用；调用结果一律 `if let` / `?` 处理

**允许的 `'static`**：字符串字面量类型（`&'static str` 错误信息、方言 SQL 常量表）、`OnceLock` 全局单例。

**审查信号**：函数签名出现两个以上显式生命周期参数时，停下来想想是否真的需要借用。

## 4. 锁与异步纪律

- `std::sync::MutexGuard` 不跨 `await`（非 Send）；取数用小作用域块
- 短同步内存临界区用 `std::sync::Mutex`（持锁时间极短）；**确需跨 `await` 独占资源的 IO 用异步锁或「任务所有者 + 消息通道」，不按锁的类型名选锁**（2026-09-12 修订）；同步锁一律不得跨 `await`
- 嵌套锁注意析构顺序（见 conn.rs get_sftp_session 的 `cached_slot` 写法）
- **async 上下文禁止同步阻塞调用**（v1.1 新增）：`std::fs::*` / `std::thread::sleep` / 阻塞网络库会卡住 tokio worker。新代码用 `tokio::fs` 或 `tokio::task::spawn_blocking`；存量触碰随手迁移。**存量规模只作参考不作门禁**（2026-09-14 扫描 `std::fs` 引用 395 处，含测试辅助与同步上下文，未逐处判定 async 可达），不因数字高就批量改动。
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

- 诊断日志一律 `eprintln!` 并带 `[模块]` 前缀（如 `[vault]` `[ssh]`），便于在 dev 控制台过滤
- **协议 stdout 与诊断通道分离**：stdout 让给子进程协议与 CLI 输出，应用诊断不写 stdout（stdout 会污染子进程协议与控制台管道，与安装包文件无关——旧表述「println 污染打包产物」不准确）
- 日志必须带级别、脱敏（同 §5）并有界保存；高频路径（轮询/逐行输出）不打日志，或只打降级后的摘要。**当前落地范围（2026-09-14 复核）**：前端已有有界错误清单与本地诊断报告（可靠性 T11），Rust 侧仍是 `eprintln!` + `[模块]` 前缀，**级别体系、落盘位置与轮转尚未落地**；本规范固定「分离、脱敏、有界」三条底线，未落地部分不作为已实现事实引用。

## 8. 依赖引入评审（v1.1 新增）

新加 crate 前回答以下问题（写进提交说明）：

1. 安全与维护：有无已知漏洞、是否仍在维护、issue 响应是否正常（**不以「最近提交时间」一票否决**，成熟低变动的库可以是正确选择）
2. 许可证：MIT/Apache-2.0/BSD 兼容？
3. 传递依赖与编译成本：依赖树多大，构建与包体代价多少？
4. 平台与封装成本：是否单平台？若是，能否封装在平台适配器内并保证另一平台可编译或显式降级（**产品必须双平台可用，但不要求每个依赖本身跨平台**）——2026-09-12 修订，取代旧「禁单平台方案」铁律
5. 与自研的总成本对比：协议/加密/解包一类能力自研的长期维护成本通常更高，避免「能不用就不用」式的过度克制

优先复用已引入生态（tokio / serde / russh / reqwest / reka）；新增 dev-only 依赖同样按此评审并记录用途与增量的包/feature。

## 9. 注释与验证（既定规则重申）

- **注释按语义强制（2026-09-12 修订）**：必须写的是——(a) 文件头模块职责；(b) `pub` / `pub(crate)` API 用途；(c) IPC 与持久化 DTO 的非显然字段（单位 / 空值语义 / 敏感性）；(d) 关键生命周期、锁与安全不变量。私有且语义显然的函数、测试辅助不强制；工具只做覆盖检查（`scripts/check_docs.py`，AR02 起并入 syn 检查器），注释质量由人工审查
- 合入门槛与命令只见[20验证矩阵](20-验证矩阵.md)，本节描述规则覆盖，不重复执行同一扫描。

## 10. 提交前检查（不做编译期强约束，2026-09-05 用户决策）

提交前运行：

```bash
python scripts/check_rust_rules.py      # 代码规则（panic / 生命周期逃逸 / unsafe 论证 / 层级越界）
python scripts/check_docs.py            # 文档覆盖（//! 文件头、pub API、DTO 字段）
node scripts/check_frontend_deps.mjs    # 前端依赖守卫（等价 pnpm check:deps）
```

**实现位置（AR02 起）**：两个 Python 脚本只是薄 wrapper，真正的检查在 `src-tauri/tests/source_rules/`（syn 2 AST + 夹具，约定入口 `scan_rust_rules` 与 `scan_docs`），CI 显式执行 `cargo test --test source_rules`。wrapper 用 `--exact` 精确过滤并**确认真的执行了 1 个测试**——过滤器写错导致 0 个测试不得算通过。

- **panic 风险**采用分类棘轮基线（`scripts/rust_rules_baseline.json` 的 `panic_by_kind`）：`unwrap` / `expect` / `panic!` / `unreachable!` / `todo!` / `assert!` 系列分类登记，只许减不许增；新增发现必须分类整改，不允许抬高数字掩盖
- **clone 与文件规模只报告、不拦截**（2026-09-12 修订）：不因数字高就批量改 `Arc` 或拆文件，改动由评审按 §2 判断
- 生命周期类（`Box::leak` / `transmute` / `mem::forget`）**零容忍无基线**，出现即失败
- `unsafe` 本身不算违规，但**每处必须带紧邻的 `// SAFETY:` 注释**（逐处校验注释块，缺失即失败；属性行可以夹在两者之间）
- §1 的合法例外必须绑定**仓库锚定相对路径 + 归属符号 + 调用类别 + 原因**（基线文件 `exceptions` 数组，不再用同一句 `expect` 文案全局放行）；符号移动或删除即失效并报错
- **独立用例文件的 panic 计数（2026-09-15 实测）**：`*_tests.rs` 被 `#[path]` 引入时扫描器按单文件独立分析，看不到引入处的 `#[cfg(test)]`，文件里的 `assert!` 会照常计入 panic 候选并触发棘轮失败。写法是文件内再包一层 `#[cfg(test)] mod …`；`is_test_only_file` 的豁免只对架构守卫（paths_bypass / foreign_table）生效，不覆盖 panic 候选
- **覆盖边界（工具自己在报告里声明，不声称语义全覆盖）**：不做 cfg 真假求值（只按属性 AST 字面排除直接 `#[cfg(test)]` 子树，`cfg(all(test, …))` / `cfg(not(test))` 一律保守扫描并列入未覆盖项）、不展开宏（`macro_rules!` 体内含候选时逐个提示，第三方/派生宏只汇总数量）、层级规则只解析 `crate::plugins::<owner>` 绝对路径（`super::` 拼出的跨插件引用不在范围）；枚举变体与 trait 实现关联项不强制文档

## 附录 · 参考来源（2026-09-05 对照验证）

本规范与以下社区公认标杆交叉验证过：

| 来源 | 星数 | 与本规范的关系 |
|------|------|---------------|
| [rust-unofficial/patterns](https://github.com/rust-unofficial/patterns) | 8.9k★ | 官方反模式第一条「Clone to satisfy the borrow checker」= 本规范 §2 的原始出处；「`#[deny(warnings)]` 是反模式」印证我们用 CLI 的 `-D warnings` 而非代码内 deny 的做法正确 |
| [rust-lang/api-guidelines](https://github.com/rust-lang/api-guidelines) | 1.3k★ | 官方 API 清单；本规范 §1 比官方更严（官方允许库代码 unwrap，我们因 IPC 直连用户而收紧），方向一致 |
| [pretzelhammer/rust-blog](https://github.com/pretzelhammer/rust-blog) | 8.4k★ | 《Common Rust Lifetime Misconceptions》= §3 的理论依据 |
| [google/comprehensive-rust](https://github.com/google/comprehensive-rust) | 33k★ | Google Android 团队课程，错误处理章节与 §5 一致 |
| [rust-lang/rust-analyzer](https://github.com/rust-lang/rust-analyzer) | 16.8k★ | 其 dev 风格指南的锁与异步纪律 = §4 出处 |
