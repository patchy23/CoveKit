//! SSH 插件 · 真实服务器集成测试（#[ignore] 默认跳过，需环境变量）
//! 用法：
//!   SSH_TEST_HOST=... SSH_TEST_USER=root SSH_TEST_PASSWORD=... \
//!   cargo test --test ssh_live -- --ignored --nocapture
//! 覆盖：密码认证连接、PTY 终端通道（写→读回显）、SFTP 目录列表。
//! 注意：本测试不落任何凭证，仅运行时内存使用。

use std::env;

use russh::{client, ChannelMsg};
use russh_sftp::client::SftpSession;

/// 读取环境变量（缺失时直接 panic，测试仅在有服务器时运行）
fn live_env() -> (String, u16, String, String) {
    let host = env::var("SSH_TEST_HOST").expect("SSH_TEST_HOST 未设置");
    let port = env::var("SSH_TEST_PORT").unwrap_or_else(|_| "22".into());
    let user = env::var("SSH_TEST_USER").unwrap_or_else(|_| "root".into());
    let password = env::var("SSH_TEST_PASSWORD").expect("SSH_TEST_PASSWORD 未设置");
    (host, port.parse().expect("端口格式错误"), user, password)
}

/// 最小 Handler（接受任意服务器密钥，指纹校验为 P2 项）
#[derive(Clone)]
struct TestHandler;

impl client::Handler for TestHandler {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _: &russh::keys::PublicKey) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// 建立连接并完成密码认证
async fn connect(host: &str, port: u16, user: &str, password: &str) -> client::Handle<TestHandler> {
    let config = Arc::new(client::Config::default());
    let mut session = client::connect(config, format!("{host}:{port}").as_str(), TestHandler)
        .await
        .expect("TCP 连接失败");
    let auth = session
        .authenticate_password(user, password)
        .await
        .expect("认证请求失败");
    assert!(auth.success(), "密码认证被拒绝");
    session
}

use std::sync::Arc;

#[tokio::test]
#[ignore = "需要真实服务器（SSH_TEST_* 环境变量）"]
async fn live_password_auth_and_exec() {
    let (host, port, user, password) = live_env();
    let session = connect(&host, port, &user, &password).await;

    // exec 命令并收集输出
    let mut channel = session.channel_open_session().await.expect("打开通道失败");
    channel
        .exec(false, "whoami && uname -s && echo OK")
        .await
        .expect("exec 失败");
    let mut out = String::new();
    while let Some(msg) = channel.wait().await {
        match msg {
            ChannelMsg::Data { data } => out.push_str(&String::from_utf8_lossy(&data)),
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    assert_eq!(out.trim(), format!("{user}\nLinux\nOK"));
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "测试结束", "")
        .await;
    println!(
        "✓ 密码认证 + exec 通过：{}",
        out.trim().replace('\n', " | ")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "需要真实服务器（SSH_TEST_* 环境变量）"]
async fn live_pty_channel_echo() {
    let (host, port, user, password) = live_env();
    let session = connect(&host, port, &user, &password).await;

    let mut channel = session.channel_open_session().await.expect("打开通道失败");
    channel
        .request_pty(false, "xterm", 100, 30, 0, 0, &[])
        .await
        .expect("PTY 请求失败");
    channel.request_shell(false).await.expect("shell 请求失败");

    // 先读取 shell 欢迎输出（确保 shell 就绪），再写入命令
    let mut out = String::new();
    for _ in 0..5 {
        if let Some(msg) = tokio::time::timeout(std::time::Duration::from_secs(5), channel.wait())
            .await
            .expect("等待 shell 就绪超时")
        {
            match msg {
                ChannelMsg::Data { data } => out.push_str(&String::from_utf8_lossy(&data)),
                ChannelMsg::Eof | ChannelMsg::Close => break,
                _ => {}
            }
        } else {
            break; // 通道关闭
        }
        if !out.is_empty() {
            break; // 已有输出，shell 就绪
        }
    }

    // 写入命令并等待回显（echo 标记便于断言）
    channel
        .data_bytes(b"echo PATCHYBOX_LIVE_TEST_2026\nexit\n".to_vec())
        .await
        .expect("写入失败");

    // 整体 15s 超时收集输出
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_secs(5), channel.wait())
            .await
            .expect("读取通道输出超时")
        {
            Some(ChannelMsg::Data { data }) => {
                out.push_str(&String::from_utf8_lossy(&data));
                if out.contains("PATCHYBOX_LIVE_TEST_2026") {
                    break;
                }
            }
            Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
            _ => {}
        }
    }
    assert!(
        out.contains("PATCHYBOX_LIVE_TEST_2026"),
        "PTY 回显未收到，输出: {}",
        out.chars().take(300).collect::<String>()
    );
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "测试结束", "")
        .await;
    println!("✓ PTY 通道写读回显通过（收到 {} 字节）", out.len());
}

#[tokio::test]
#[ignore = "需要真实服务器（SSH_TEST_* 环境变量）"]
async fn live_sftp_list_root() {
    let (host, port, user, password) = live_env();
    let session = connect(&host, port, &user, &password).await;

    let channel = session.channel_open_session().await.expect("打开通道失败");
    channel
        .request_subsystem(false, "sftp")
        .await
        .expect("SFTP 子系统请求失败");
    let stream = channel.into_stream();
    let sftp = SftpSession::new(stream).await.expect("SFTP 初始化失败");

    let entries = sftp.read_dir("/").await.expect("read_dir 失败");
    let names: Vec<String> = entries.map(|e| e.file_name().to_string()).collect();
    assert!(!names.is_empty(), "根目录列表为空");
    assert!(
        names
            .iter()
            .any(|n| n == "root" || n == "etc" || n == "home"),
        "根目录缺少常见目录: {names:?}"
    );
    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "测试结束", "")
        .await;
    println!("✓ SFTP 目录列表通过（{} 项）", names.len());
}
