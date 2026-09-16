//! 本地数据导出导入（sync-202609-001 L2）：数据包容器、可导出目录、隔离导入
//!
//! 分工（方案 §5.1）：
//! - `package` / `types`：C1 容器层——`.pbdata` 读写、清单与 DTO、选择集校验
//! - `adapter` / `catalog` / `datasets`：C2 导出链路——owner 适配器登记、选择集闭包、导出目录
//! - `import`：C3 隔离导入——写暂存目录、一次 rename、索引登记与回滚
//! - 命令层（§5.3 的 8 条冻结命令）在 C4 追加
//!
//! 三条边界（与 L2 方案 §4.3 一致）：
//! - 秘密只经加密包搬移：`secret` 策略的块仅在用户确认勾选时携带记录，未勾选时只声明条数
//! - 明文不进 JS：解密、解析、生成全在 Rust 侧
//! - 口令不落盘：口令只作为函数参数在单次调用内存活，不写设置文件、不进日志

#[allow(dead_code)] // C4 命令层接入前，导出链路只有测试调用方（接入后删掉本行）
pub(crate) mod adapter;
#[allow(dead_code)] // 同上
pub(crate) mod catalog;
#[allow(dead_code)] // 同上
pub(crate) mod datasets;
#[allow(dead_code)] // 同上
pub(crate) mod import;
#[allow(dead_code)] // C4 命令层接入前，容器与 DTO 只有测试调用方
pub(crate) mod package;
pub(crate) mod types;
