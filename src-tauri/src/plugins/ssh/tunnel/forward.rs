//! SSH 隧道 · 数据面（监听接受 / SOCKS5 / 双向互拷）

use std::sync::{atomic::Ordering, Arc};

use russh::client::Handle;
use tauri::AppHandle;
use tokio::{io::copy_bidirectional, net::TcpListener, sync::watch};

use crate::plugins::ssh::conn::SshHandler;
use crate::plugins::ssh::models::TunnelStatus;

use super::TunnelHandle;

/// 本地转发（-L）：accept → direct-tcpip → 双向互拷
pub(crate) fn spawn_local_forward(
    app: AppHandle,
    handle: Arc<TunnelHandle>,
    session: Arc<Handle<SshHandler>>,
    listener: TcpListener,
    mut cancel_rx: watch::Receiver<bool>,
    target_host: String,
    target_port: u16,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        loop {
            let accepted = tokio::select! {
                _ = cancel_rx.changed() => break,
                accepted = listener.accept() => accepted,
            };
            let (tcp, peer) = match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    handle.set_status(
                        &app,
                        TunnelStatus::Error,
                        Some(format!("接受连接失败: {e}")),
                    );
                    break;
                }
            };
            let session = session.clone();
            let counter = handle.connections.clone();
            let target_host = target_host.clone();
            counter.fetch_add(1, Ordering::Relaxed);
            tauri::async_runtime::spawn(async move {
                let pipe = async {
                    let channel = session
                        .channel_open_direct_tcpip(
                            target_host.as_str(),
                            u32::from(target_port),
                            peer.ip().to_string(),
                            u32::from(peer.port()),
                        )
                        .await
                        .map_err(|e| format!("打开转发通道失败: {e}"))?;
                    let mut stream = channel.into_stream();
                    let mut tcp = tcp;
                    copy_bidirectional(&mut stream, &mut tcp)
                        .await
                        .map_err(|e| format!("转发中断: {e}"))?;
                    Ok::<(), String>(())
                };
                let _ = pipe.await;
                counter.fetch_sub(1, Ordering::Relaxed);
            });
        }
    })
}

/// 动态转发（-D）：SOCKS5 握手后按客户端请求 direct-tcpip
pub(crate) fn spawn_dynamic_forward(
    handle: Arc<TunnelHandle>,
    session: Arc<Handle<SshHandler>>,
    listener: TcpListener,
    mut cancel_rx: watch::Receiver<bool>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        loop {
            let accepted = tokio::select! {
                _ = cancel_rx.changed() => break,
                accepted = listener.accept() => accepted,
            };
            let (tcp, peer) = match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    let _ = &handle;
                    let _ = e;
                    break;
                }
            };
            let session = session.clone();
            let counter = handle.connections.clone();
            counter.fetch_add(1, Ordering::Relaxed);
            tauri::async_runtime::spawn(async move {
                let _ = serve_socks5(session, tcp, peer.port()).await;
                counter.fetch_sub(1, Ordering::Relaxed);
            });
        }
    })
}

/// 单个 SOCKS5 客户端的最小实现：无认证 + CONNECT 命令
async fn serve_socks5(
    session: Arc<Handle<SshHandler>>,
    mut tcp: tokio::net::TcpStream,
    peer_port: u16,
) -> Result<(), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut head = [0u8; 2];
    tcp.read_exact(&mut head)
        .await
        .map_err(|e| format!("读取 SOCKS5 握手失败: {e}"))?;
    if head[0] != 5 {
        return Err("不是 SOCKS5 协议".into());
    }
    let methods = head[1] as usize;
    let mut skip = vec![0u8; methods];
    tcp.read_exact(&mut skip)
        .await
        .map_err(|e| format!("读取认证方法失败: {e}"))?;
    tcp.write_all(&[5, 0])
        .await
        .map_err(|e| format!("应答认证失败: {e}"))?;

    let mut req = [0u8; 4];
    tcp.read_exact(&mut req)
        .await
        .map_err(|e| format!("读取请求失败: {e}"))?;
    if req[1] != 1 {
        reply_socks5(&mut tcp, 0x07).await?;
        return Err("仅支持 CONNECT 命令".into());
    }
    let host = match req[3] {
        1 => {
            let mut buf = [0u8; 4];
            tcp.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
            std::net::Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]).to_string()
        }
        3 => {
            let mut len = [0u8; 1];
            tcp.read_exact(&mut len).await.map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; len[0] as usize];
            tcp.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
            String::from_utf8(buf).map_err(|_| "域名不是合法 UTF-8".to_string())?
        }
        4 => {
            let mut buf = [0u8; 16];
            tcp.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
            std::net::Ipv6Addr::from(buf).to_string()
        }
        other => {
            reply_socks5(&mut tcp, 0x08).await?;
            return Err(format!("不支持的地址类型 {other}"));
        }
    };
    let mut port_buf = [0u8; 2];
    tcp.read_exact(&mut port_buf)
        .await
        .map_err(|e| e.to_string())?;
    let port = u16::from_be_bytes(port_buf);

    let channel = match session
        .channel_open_direct_tcpip(
            host.as_str(),
            u32::from(port),
            "127.0.0.1",
            u32::from(peer_port),
        )
        .await
    {
        Ok(channel) => channel,
        Err(e) => {
            reply_socks5(&mut tcp, 0x01).await?;
            return Err(format!("打开转发通道失败: {e}"));
        }
    };
    reply_socks5(&mut tcp, 0x00).await?;
    let mut stream = channel.into_stream();
    copy_bidirectional(&mut stream, &mut tcp)
        .await
        .map_err(|e| format!("转发中断: {e}"))?;
    Ok(())
}

/// SOCKS5 应答（仅状态字节有意义的极简包）
async fn reply_socks5(tcp: &mut tokio::net::TcpStream, code: u8) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    tcp.write_all(&[5, code, 0, 0, 0, 0, 0, 0, 0, 0])
        .await
        .map_err(|e| format!("SOCKS5 应答失败: {e}"))
}
