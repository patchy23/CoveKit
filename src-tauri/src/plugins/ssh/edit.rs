//! SSH 插件 · 远程编辑（下载 → 本地编辑 → 上传回写）
//! 前端用 CodeMirror 编辑（临时文件或内存），保存时调用 ssh_edit_save 回写。

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use tauri::State;

use crate::plugins::ssh::conn::{get_session, resource_id, SshState};
use crate::plugins::ssh::file::replace_remote_file;
use crate::plugins::ssh::models::{RemoteFileContent, SshActionResult};

/// 远程编辑器最大文件大小，避免一次性读取超大文件耗尽内存。
const MAX_EDIT_BYTES: u64 = 5 * 1024 * 1024;

/// 临时 SFTP 会话（与 file.rs 同构，避免跨文件依赖）
async fn sftp_session(
    ssh_state: &State<'_, SshState>,
    connection_id: &str,
) -> Result<russh_sftp::client::SftpSession, String> {
    let session = get_session(ssh_state, connection_id)?;
    let channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("打开通道失败: {e}"))?;
    channel
        .request_subsystem(false, "sftp")
        .await
        .map_err(|e| format!("SFTP 子系统请求失败: {e}"))?;
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
    let file = sftp
        .open(&remote_path)
        .await
        .map_err(|e| format!("打开文件失败: {e}"))?;
    if meta.size.unwrap_or(0) > MAX_EDIT_BYTES {
        return Err("远程编辑仅支持不超过 5 MiB 的文本文件".into());
    }
    let mut buf = Vec::new();
    file.take(MAX_EDIT_BYTES + 1)
        .read_to_end(&mut buf)
        .await
        .map_err(|e| format!("读取内容失败: {e}"))?;
    if buf.len() as u64 > MAX_EDIT_BYTES {
        return Err("远程编辑仅支持不超过 5 MiB 的文本文件".into());
    }
    let size = meta.size.unwrap_or(buf.len() as u64);
    // 编辑器仅支持 UTF-8；拒绝有损解码，避免保存时静默破坏原文件字节。
    let content = String::from_utf8(buf)
        .map_err(|_| "文件不是有效 UTF-8 文本，暂不支持直接编辑".to_string())?;
    Ok(RemoteFileContent {
        ok: true,
        path: remote_path,
        content,
        size,
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
    let target_path = sftp
        .canonicalize(&remote_path)
        .await
        .map_err(|e| format!("解析远程路径失败: {e}"))?;
    let metadata = sftp
        .metadata(&target_path)
        .await
        .map_err(|e| format!("读取远程文件属性失败: {e}"))?;
    let temp_path = format!("{target_path}.patchybox-edit-{}", resource_id("file"));
    let mut file = sftp
        .create(&temp_path)
        .await
        .map_err(|e| format!("创建文件失败: {e}"))?;
    file.write_all(content.as_bytes())
        .await
        .map_err(|e| format!("写入失败: {e}"))?;
    file.flush()
        .await
        .map_err(|e| format!("刷新文件失败: {e}"))?;
    file.shutdown()
        .await
        .map_err(|e| format!("关闭远程文件失败: {e}"))?;
    sftp.set_metadata(
        &temp_path,
        russh_sftp::protocol::FileAttributes {
            permissions: metadata.permissions,
            ..russh_sftp::protocol::FileAttributes::default()
        },
    )
    .await
    .map_err(|e| format!("恢复远程文件属性失败: {e}"))?;
    replace_remote_file(&sftp, &temp_path, &target_path).await?;
    Ok(SshActionResult {
        ok: true,
        error: None,
    })
}
