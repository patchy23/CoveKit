//! 数据传输命令装配与任务取消入口。

pub(crate) mod export;
pub(crate) mod import;
pub(crate) mod spaces;
pub(crate) mod views;
use crate::framework::data_transfer::session;

use self::views::CancelResult;

/// 传输任务的任务类型（任务面板显示用）
pub(super) const TASK_KIND: &str = "data-transfer";

/// 导出任务标签
pub(super) const TASK_LABEL_EXPORT: &str = "导出数据包";

/// 导入任务标签
pub(super) const TASK_LABEL_IMPORT: &str = "导入数据包";

/// 取消正在进行的导出 / 导入
#[tauri::command]
pub fn data_transfer_cancel() -> CancelResult {
    CancelResult {
        cancelled: session::cancel_current(),
    }
}
