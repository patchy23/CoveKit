//! 文件占用查询的传输契约，与前端 file-lock/contracts.ts 同步。

use serde::Serialize;

/// 使用目标文件的进程快照，不代表该进程必然阻止删除。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProcess {
    pub(crate) pid: u32,
    /// Windows FILETIME 的十进制字符串，与 PID 共同标识进程，避免 JS 精度丢失。
    pub(crate) started_at: String,
    pub(crate) app_name: String,
    /// 非服务进程为 null。
    pub(crate) service_name: Option<String>,
    /// 无权限、已退出或身份变化时为 null，原因见 detail_error。
    pub(crate) executable_path: Option<String>,
    pub(crate) process_name: Option<String>,
    /// 进程补充信息读取失败的可见原因；查询到的基础信息仍保留。
    pub(crate) detail_error: Option<String>,
}

/// 单文件查询结果；空列表仅表示本次未发现使用者。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileLockResult {
    pub(crate) path: String,
    pub(crate) processes: Vec<FileProcess>,
}
