//! SSH 隧道 · 数据面（监听接受 / SOCKS5 / 双向互拷）

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use russh::client::Handle;
use tauri::AppHandle;
use tokio::{io::copy_bidirectional, net::TcpListener, sync::watch};

use crate::plugins::ssh::conn::SshHandler;
use crate::plugins::ssh::models::TunnelStatus;

use super::TunnelHandle;

/// 连接计数随任务作用域回收，取消或 panic 不依赖执行到函数末尾。
pub(crate) struct ConnectionGuard(Arc<AtomicU64>);

impl ConnectionGuard {
    pub(crate) fn new(counter: Arc<AtomicU64>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self(counter)
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

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
        let mut children = tokio::task::JoinSet::new();
        loop {
            if *cancel_rx.borrow() {
                break;
            }
            let accepted = tokio::select! {
                _ = cancel_rx.changed() => break,
                completed = children.join_next(), if !children.is_empty() => {
                    if matches!(completed, Some(Err(_))) {
                        log::warn!("本地转发子任务异常退出");
                    }
                    continue;
                }
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
            let connection = ConnectionGuard::new(counter);
            children.spawn(async move {
                let _connection = connection;
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
                if pipe.await.is_err() {
                    log::debug!("本地转发连接结束或建立失败");
                }
            });
        }
        // 停止监听同时回收已接受连接；监听任务被 abort 时 JoinSet 的 Drop 也会取消子任务。
        children.shutdown().await;
    })
}

/// 动态转发（-D）：SOCKS5 握手后按客户端请求 direct-tcpip
pub(crate) fn spawn_dynamic_forward(
    app: AppHandle,
    handle: Arc<TunnelHandle>,
    session: Arc<Handle<SshHandler>>,
    listener: TcpListener,
    mut cancel_rx: watch::Receiver<bool>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut children = tokio::task::JoinSet::new();
        loop {
            if *cancel_rx.borrow() {
                break;
            }
            let accepted = tokio::select! {
                _ = cancel_rx.changed() => break,
                completed = children.join_next(), if !children.is_empty() => {
                    if matches!(completed, Some(Err(_))) {
                        log::warn!("SOCKS5 转发子任务异常退出");
                    }
                    continue;
                }
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
            let connection = ConnectionGuard::new(counter);
            children.spawn(async move {
                let _connection = connection;
                if serve_socks5(session, tcp, peer.port()).await.is_err() {
                    log::debug!("SOCKS5 转发连接结束或建立失败");
                }
            });
        }
        children.shutdown().await;
    })
}

/// 单个 SOCKS5 客户端的最小实现：无认证 + CONNECT 命令
async fn serve_socks5(
    session: Arc<Handle<SshHandler>>,
    mut tcp: tokio::net::TcpStream,
    peer_port: u16,
) -> Result<(), String> {
    let mut stream = open_socks5_stream(session, &mut tcp, peer_port).await?;
    copy_bidirectional(&mut stream, &mut tcp)
        .await
        .map_err(|e| format!("转发中断: {e}"))?;
    Ok(())
}

/// 读取 SOCKS5 协商目标；尚未请求 SSH 通道，超时可直接释放本地 socket。
async fn read_socks5_target<S>(tcp: &mut S) -> Result<(String, u16), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
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
        reply_socks5(tcp, 0x07).await?;
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
            reply_socks5(tcp, 0x08).await?;
            return Err(format!("不支持的地址类型 {other}"));
        }
    };
    let mut port_buf = [0u8; 2];
    tcp.read_exact(&mut port_buf)
        .await
        .map_err(|e| e.to_string())?;
    let port = u16::from_be_bytes(port_buf);
    Ok((host, port))
}

/// 完成 SOCKS5 协商并返回带 Drop 关闭责任的 SSH 流。
async fn open_socks5_stream(
    session: Arc<Handle<SshHandler>>,
    tcp: &mut tokio::net::TcpStream,
    peer_port: u16,
) -> Result<russh::ChannelStream<russh::client::Msg>, String> {
    // 期限只覆盖客户端握手，不截断已建立的空闲或长时间传输连接。
    let (host, port) = tokio::time::timeout(Duration::from_secs(30), read_socks5_target(tcp))
        .await
        .map_err(|_| "SOCKS5 握手超时".to_string())??;
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
            reply_socks5(tcp, 0x01).await?;
            return Err(format!("打开转发通道失败: {e}"));
        }
    };
    let stream = channel.into_stream();
    // 应答失败或任务被取消时也由 stream 的 Drop 负责关闭 SSH 通道。
    reply_socks5(tcp, 0x00).await?;
    Ok(stream)
}

/// SOCKS5 应答（仅状态字节有意义的极简包）
async fn reply_socks5<S: tokio::io::AsyncWrite + Unpin>(tcp: &mut S, code: u8) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    tcp.write_all(&[5, code, 0, 0, 0, 0, 0, 0, 0, 0])
        .await
        .map_err(|e| format!("SOCKS5 应答失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 仅用内存流验证握手完整性，不连接真实 SSH 或 TCP 服务。
    #[tokio::test]
    async fn socks_target_keeps_domain_and_port() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut client, mut server) = tokio::io::duplex(64);
        let request = async {
            client.write_all(&[5, 1, 0]).await.unwrap();
            let mut reply = [0; 2];
            client.read_exact(&mut reply).await.unwrap();
            assert_eq!(reply, [5, 0]);
            client
                .write_all(b"\x05\x01\x00\x03\x0bexample.com\x01\xbb")
                .await
                .unwrap();
        };
        let (target, ()) = tokio::join!(read_socks5_target(&mut server), request);
        assert_eq!(target.unwrap(), ("example.com".into(), 443));
    }

    /// 半包客户端不会让握手期限失效；读取期间没有打开 SSH 通道。
    #[tokio::test]
    async fn incomplete_socks_handshake_can_time_out() {
        use tokio::io::AsyncWriteExt;
        let (mut client, mut server) = tokio::io::duplex(64);
        client.write_all(&[5]).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(10), read_socks5_target(&mut server))
                .await
                .is_err()
        );
    }

    /// 即使子任务在第一次轮询前就被取消，已登记连接也必须归零。
    #[tokio::test]
    async fn cancellation_releases_unpolled_connections() {
        let counter = Arc::new(AtomicU64::new(0));
        let mut children = tokio::task::JoinSet::new();
        for _ in 0..100 {
            let connection = ConnectionGuard::new(Arc::clone(&counter));
            children.spawn(async move {
                let _connection = connection;
                std::future::pending::<()>().await;
            });
        }
        assert_eq!(counter.load(Ordering::Relaxed), 100);
        children.shutdown().await;
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }

    /// 正常完成和已进入复制阶段的取消都使用同一计数回收责任。
    #[tokio::test]
    async fn completion_and_cancellation_release_active_connections() {
        let counter = Arc::new(AtomicU64::new(0));
        let mut children = tokio::task::JoinSet::new();
        let connection = ConnectionGuard::new(Arc::clone(&counter));
        children.spawn(async move {
            let _connection = connection;
        });
        children.join_next().await.unwrap().unwrap();
        assert_eq!(counter.load(Ordering::Relaxed), 0);
        let connection = ConnectionGuard::new(Arc::clone(&counter));
        let (started, ready) = tokio::sync::oneshot::channel();
        children.spawn(async move {
            let _connection = connection;
            started.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        ready.await.unwrap();
        children.shutdown().await;
        assert_eq!(counter.load(Ordering::Relaxed), 0);
    }
}
