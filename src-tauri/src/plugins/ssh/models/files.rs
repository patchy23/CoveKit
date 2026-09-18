//! SSH 数据模型 · files

use serde::Serialize;

/* ── 文件管理 ── */

/// 远程文件条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    /// 文件名
    pub(crate) name: String,
    /// 完整路径
    pub(crate) path: String,
    /// 是否目录
    pub(crate) is_dir: bool,
    /// 文件大小（字节，目录为 0）
    pub(crate) size: u64,
    /// 修改时间（毫秒时间戳）
    pub(crate) modified_at: u64,
    /// 权限字符串（如 drwxr-xr-x）
    pub(crate) permissions: String,
    /// 所有者
    pub(crate) owner: String,
    /// 所属组
    pub(crate) group: String,
}

/// 文件列表结果
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileListResult {
    /// 查询是否成功
    pub(crate) ok: bool,
    /// 当前路径
    pub(crate) path: String,
    /// 父路径（根目录为 null）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_path: Option<String>,
    /// 文件列表
    pub(crate) files: Vec<RemoteFile>,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/// 文件传输进度
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTransferProgress {
    /// 传输 id
    pub(crate) transfer_id: String,
    /// 所属 SSH 连接会话 id
    pub(crate) connection_id: String,
    /// 本地路径
    pub(crate) local_path: String,
    /// 远程路径
    pub(crate) remote_path: String,
    /// 已传输字节
    pub(crate) transferred: u64,
    /// 总字节
    pub(crate) total: u64,
    /// 是否完成
    pub(crate) done: bool,
    /// 是否失败
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/* ── 远程编辑 ── */

/// 远程文件内容
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileContent {
    /// 读取是否成功
    pub(crate) ok: bool,
    /// 文件路径
    pub(crate) path: String,
    /// 文件内容（文本）
    pub(crate) content: String,
    /// 文件大小（字节）
    pub(crate) size: u64,
    /// 编码（如 UTF-8 / GBK）
    pub(crate) encoding: String,
    /// 修改时间（毫秒；编辑器乐观锁基线）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) modified_at: Option<u64>,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

/// 远程编辑保存结果（冲突时带当前 mtime 供前端决策）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditSaveResult {
    /// 保存是否成功
    pub(crate) ok: bool,
    /// 失败原因（成功时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    /// 远端文件已被他人修改（乐观锁冲突），未写入
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) conflict: Option<bool>,
    /// 冲突时远端当前 mtime（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_mtime: Option<u64>,
    /// 冲突时远端当前内容（供前端展示差异对比；读取失败或超限时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) remote_content: Option<String>,
}
