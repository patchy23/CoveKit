//! 本地数据导出导入（sync-202609-001 L2/L3）：数据包容器、导出目录、隔离导入与合并/覆盖导入
//!
//! 分工：
//! - `package` / `types`：容器层——`.pbdata` 读写、清单与 DTO、选择集校验
//! - `adapter` / `catalog` / `datasets`：导出链路——owner 适配器登记、选择集闭包、导出目录
//! - `import`：隔离导入——写暂存目录、一次 rename、索引登记与回滚
//! - `lineage` / `merge` / `backup`：合并/覆盖导入——导入映射、原地事务提交、导入前快照
//! - `commands`：命令层（§5.3 的冻结命令 + 快照还原两条）
//!
//! 三条边界：
//! - 凭证永不进包（与空间 uid 绑死）：`secret` 策略的块只声明条数，不携带记录
//! - 明文不进 JS：解密、解析、生成全在 Rust 侧
//! - 口令不落盘：口令只作为函数参数在单次调用内存活，不写设置文件、不进日志

use tauri::Emitter;

pub(crate) mod adapter;
/// 导入前快照：将被修改的存储的字节级备份与还原（L3）
pub(crate) mod backup;
pub(crate) mod catalog;
/// 命令层（§5.3 的 8 条冻结命令）与请求/响应 DTO
pub(crate) mod commands;
pub(crate) mod datasets;
pub(crate) mod import;
/// 导入映射（sourceLineage）：重复导入的幂等依据（L3）
pub(crate) mod lineage;
/// 合并/覆盖导入提交：原地物化（L3）
pub(crate) mod merge;
pub(crate) mod package;
/// owner 逻辑记录的公共传输编排。
pub(crate) mod records;
/// 会话上下文（inspectId / planId）与取消标志
mod session;
mod storage_files;
pub(crate) mod types;

/// 提交成功后广播受影响数据集（前端订阅后定点重拉，合并/覆盖导入不重启生效的落点）
pub(crate) fn emit_space_data_changed(app: &tauri::AppHandle, datasets: &[String]) {
    let _ = app.emit(
        "space-data-changed",
        serde_json::json!({ "datasets": datasets }),
    );
}
