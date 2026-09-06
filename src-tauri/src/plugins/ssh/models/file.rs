//! SSH 数据模型 · 文件（条目 / 列表结果 / 传输进度 / 远程编辑）

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
}

/* ── 资源监控 ── */

/// 监控数据点
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorData {
    /// CPU 使用率 0-100
    pub(crate) cpu_percent: f32,
    /// 内存使用率 0-100
    pub(crate) memory_percent: f32,
    /// 内存已用（字节）
    pub(crate) memory_used: u64,
    /// 内存总量（字节）
    pub(crate) memory_total: u64,
    /// 磁盘使用率 0-100
    pub(crate) disk_percent: f32,
    /// 磁盘已用（字节）
    pub(crate) disk_used: u64,
    /// 磁盘总量（字节）
    pub(crate) disk_total: u64,
    /// 网络上行速率（字节/秒）
    pub(crate) net_upload_bps: u64,
    /// 网络下行速率（字节/秒）
    pub(crate) net_download_bps: u64,
    /// 采样时间（毫秒时间戳）
    pub(crate) timestamp: u64,
}

/* ── 服务管理 ── */

/// systemd 服务条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemdService {
    /// 服务名（如 nginx.service）
    pub(crate) name: String,
    /// 描述
    pub(crate) description: String,
    /// 加载状态（loaded / not-found / masked）
    pub(crate) load_state: String,
    /// 活动状态（active / inactive / failed）
    pub(crate) active_state: String,
    /// 子状态（running / dead / exited）
    pub(crate) sub_state: String,
    /// 是否开机自启
    pub(crate) enabled: bool,
}

/* ── 进程管理 ── */

/// 进程条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    /// 进程 ID
    pub(crate) pid: u32,
    /// 运行用户
    pub(crate) user: String,
    /// CPU 使用率 0-100
    pub(crate) cpu_percent: f32,
    /// 内存使用率 0-100
    pub(crate) memory_percent: f32,
    /// 内存占用（字节）
    pub(crate) memory_bytes: u64,
    /// 启动时间（毫秒时间戳）
    pub(crate) started_at: u64,
    /// 完整命令行
    pub(crate) command: String,
}

/* ── Docker ── */

/// Docker 容器条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerContainer {
    /// 容器 ID（短）
    pub(crate) id: String,
    /// 容器名
    pub(crate) name: String,
    /// 镜像名
    pub(crate) image: String,
    /// 状态（running / exited / paused）
    pub(crate) status: String,
    /// 本次运行持续时间（来自 docker ps Status）
    pub(crate) uptime: String,
    /// 端口映射（如 "80:80,443:443"）
    pub(crate) ports: String,
    /// 创建时间（毫秒时间戳）
    pub(crate) created_at: u64,
}

/// Docker 日志条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // 契约类型：docker 日志列表，当前版本未构造（v1 返回原始文本）
pub struct DockerLog {
    /// 容器 ID
    pub(crate) container_id: String,
    /// 日志内容
    pub(crate) content: String,
    /// 时间戳
    pub(crate) time: u64,
}
