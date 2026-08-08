//! SSH 插件 · 远程编辑（下载 → 本地编辑 → 上传回写）
//! 前端用 CodeMirror 编辑（临时文件或内存），保存时调用 ssh_edit_save 回写。

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tauri::State;

use crate::plugins::ssh::conn::SshState;
use crate::plugins::ssh::models::{RemoteFileContent, SshActionResult};

/// 临时 SFTP 会话（与 file.rs 同构，避免跨文件依赖）
async fn sftp_session(
    ssh_state: &State<'_, SshState>,
    connection_id: &str,
) -> Result<russh_sftp::client::SftpSession, String> {
    let session = ssh_state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(connection_id)
        .map(|h| h.session.clone())
        .ok_or("连接不存在或已断开")?;
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    let stream = channel.into_stream();
    russh_sftp::client::SftpSession::new(stream)
        .await
        .map_err(|e| format!("SFTP 初始化失败: {e}"))
}

/// 打开远程文件（下载完整内容）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_edit_open(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    remote_path: String,
) -> Result<RemoteFileContent, String> {
    let sftp = sftp_session(&ssh_state, &connection_id).await?;
    let meta = sftp
        .metadata(&remote_path)
        .await
        .map_err(|e| format!("读取元数据失败: {e}"))?;
    let mut file = sftp
        .open(&remote_path)
        .await
        .map_err(|e| format!("打开文件失败: {e}"))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .await
        .map_err(|e| format!("读取内容失败: {e}"))?;
    // 尝试按 UTF-8 解析（非 UTF-8 文件前端显示替换符，保存原样回写）
    let content = String::from_utf8_lossy(&buf).to_string();
    Ok(RemoteFileContent {
        ok: true,
        path: remote_path,
        content,
        size: meta.size.unwrap_or(buf.len() as u64),
        encoding: "UTF-8".into(),
        error: None,
    })
}

/// 保存远程文件（内容上传回写）
#[tauri::command(rename_all = "camelCase")]
pub async fn ssh_edit_save(
    ssh_state: State<'_, SshState>,
    connection_id: String,
    remote_path: String,
    content: String,
) -> Result<SshActionResult, String> {
    let sftp = sftp_session(&ssh_state, &connection_id).await?;
    let mut file = sftp
        .create(&remote_path)
        .await
        .map_err(|e| format!("创建文件失败: {e}"))?;
    file.write_all(content.as_bytes())
        .await
        .map_err(|e| format!("写入失败: {e}"))?;
    let _ = file.flush().await;
    Ok(SshActionResult {
        ok: true,
        error: None,
    })
}
