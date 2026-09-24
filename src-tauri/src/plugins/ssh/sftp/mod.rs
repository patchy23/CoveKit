//! SSH 插件 · 文件管理（SFTP 长驻会话复用，见 conn::get_sftp_session）
//! 上传/下载为后台任务分块传输，进度经事件 ssh://transfer-progress 推送；
//! 传输支持协作式取消（取消位注册表），上传/下载均走临时文件 + 原子替换。
//! 模块划分：browse = 目录浏览；transfer = 传输与文件操作 + 共享工具。

pub(crate) mod browse;
pub(crate) mod ops;
pub(crate) mod transfer;
pub(crate) mod util;

pub use ops::TransferState;

pub(crate) mod edit;

pub(crate) mod editor_window;
