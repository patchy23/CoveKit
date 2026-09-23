//! SSH 归档命令及流式事件 DTO。
use serde::{Deserialize, Serialize};

/// 受控工作器请求，所有路径由结构化输入传送而非拼入 shell。
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveRequest {
    /// compress / extract / preview
    pub operation: String,
    /// zip / gz / tar.gz
    pub format: String,
    /// 远程输入绝对路径
    pub paths: Vec<String>,
    /// 创建/解压目标；预览为空串
    pub output: String,
}

/// 归档只读目录项。
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    /// 包内相对路径
    pub path: String,
    /// 原始大小；裸 GZ 未扫描时未知
    pub size: Option<u64>,
    /// 目录标记
    pub is_dir: bool,
    /// 毫秒修改时间
    pub modified_at: u64,
}

/// 工作器增量消息；队列、进度、条目和终态使用同一命令 Channel。
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEvent {
    /// queued / progress / entries / error / result
    pub kind: String,
    /// 实际处理字节
    pub transferred: Option<u64>,
    /// 已知总量
    pub total: Option<u64>,
    /// 分批目录项
    pub entries: Option<Vec<ArchiveEntry>>,
    /// succeeded / failed / cancelled
    pub status: Option<String>,
    /// 可展示的失败信息
    pub error: Option<String>,
}
