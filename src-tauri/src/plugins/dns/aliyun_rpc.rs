//! AliDNS RPC 签名与传输。连接池不保存凭证，签名 URL 不进入错误或日志。

use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use serde::de::DeserializeOwned;
use sha1::Sha1;

use super::models::ProviderConfig;

const ENDPOINT: &str = "https://alidns.cn-hangzhou.aliyuncs.com/";

/// 每个命令持有当前配置快照；共享的只有 reqwest 连接池。
pub(super) struct AliyunRpc {
    client: reqwest::Client,
    id: String,
    secret: String,
}

impl AliyunRpc {
    pub(super) fn new(config: &ProviderConfig) -> Self {
        Self {
            client: super::http_client(),
            id: config.id.clone(),
            secret: config.key.clone(),
        }
    }

    pub(super) async fn call<T: DeserializeOwned>(
        &self,
        action: &str,
        params: &[(&str, &str)],
    ) -> Result<T, String> {
        self.call_at(ENDPOINT, action, params).await
    }

    /// 生产入口只传固定端点，独立端点参数供离线回归验证真实连接复用。
    async fn call_at<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        action: &str,
        params: &[(&str, &str)],
    ) -> Result<T, String> {
        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let nonce = uuid::Uuid::new_v4().to_string();
        let query = signed_query(&self.id, &self.secret, action, params, &timestamp, &nonce)?;
        // 与原 SDK 一致：POST 空正文，参数与签名放在查询串。取消由命令丢弃此 Future。
        let response = self.client.post(format!("{endpoint}?{query}"))
            .send().await.map_err(|error| {
                format!("阿里云请求失败: {}", error.without_url())
            })?;
        let status = response.status();
        if !status.is_success() {
            // 服务端 Message/Recommend 可能回显签名 URL 或入参，不将它们透传到诊断。
            let body = response.json::<serde_json::Value>().await
                .map_err(|error| format!("阿里云错误响应解析失败（HTTP {status}）: {}", error.without_url()))?;
            return Err(service_error(status.as_u16(), &body));
        }
        response.json().await
            .map_err(|error| format!("阿里云响应解析失败: {}", error.without_url()))
    }
}

/// RPC v1 使用 RFC3986 编码：空格为 %20，保留波浪号，对 UTF-8 字节编码。
fn encode(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut result = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            result.push(char::from(byte));
        } else {
            result.push('%');
            result.push(char::from(HEX[usize::from(byte >> 4)]));
            result.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    result
}

/// 时间和 nonce 显式传入，使签名契约可用固定夹具离线验证。
fn signed_query(
    id: &str,
    secret: &str,
    action: &str,
    query: &[(&str, &str)],
    timestamp: &str,
    nonce: &str,
) -> Result<String, String> {
    let mut params = vec![
        ("AccessKeyId", id),
        ("Action", action),
        ("Format", "JSON"),
        ("SignatureMethod", "HMAC-SHA1"),
        ("SignatureNonce", nonce),
        ("SignatureVersion", "1.0"),
        ("Timestamp", timestamp),
        ("Version", "2015-01-09"),
    ];
    params.extend_from_slice(query);
    params.sort_unstable_by_key(|(key, _)| *key);
    let canonical = params.iter().map(|(key, value)| {
        format!("{}={}", encode(key), encode(value))
    }).collect::<Vec<_>>().join("&");
    let mut mac = Hmac::<Sha1>::new_from_slice(format!("{secret}&").as_bytes())
        .map_err(|_| "阿里云签名初始化失败".to_string())?;
    mac.update(format!("POST&%2F&{}", encode(&canonical)).as_bytes());
    let signature = STANDARD.encode(mac.finalize().into_bytes());
    Ok(format!("Signature={}&{canonical}", encode(&signature)))
}

/// 仅展示协议错误标识；不允许错误正文中的任意字符串进入用户诊断。
fn service_error(status: u16, body: &serde_json::Value) -> String {
    let code = body.get("Code").and_then(serde_json::Value::as_str);
    let description = match code {
        Some("InvalidAccessKeyId.NotFound" | "InvalidAccessKeyId") => "AccessKey ID 无效",
        Some("SignatureDoesNotMatch") => "签名校验失败，请检查 AccessKey Secret",
        Some("Forbidden" | "Forbidden.RAM") => "当前凭证没有操作权限",
        Some("Throttling" | "Throttling.User") => "请求过于频繁，请稍后重试",
        Some("DomainRecordDuplicate") => "解析记录已存在",
        Some("InvalidDomainName.NoExist" | "InvalidDomainName.NotFound") => "域名不存在",
        _ => "服务拒绝请求，请检查参数及凭证权限",
    };
    format!("阿里云请求失败（HTTP {status}）: {description}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_encoding_preserves_rfc3986_semantics() {
        assert_eq!(encode(" +*~/-_中"), "%20%2B%2A~%2F-_%E4%B8%AD");
    }

    #[test]
    fn fixed_post_signature_and_credential_replacement() {
        // 独立 Python hashlib/hmac 夹具，含 Unicode、空格和保留字符。
        let params = [("DomainName", "example.com"), ("RR", "@"), ("Type", "TXT"),
            ("Value", "中文 +*~/"), ("TTL", "600")];
        let query = signed_query("testid", "testsecret", "AddDomainRecord", &params,
            "2026-09-27T00:00:00Z", "fixed-nonce").unwrap();
        assert_eq!(query, "Signature=YeI%2B7BRvTl88RosRIcJDakkJe%2Fs%3D&AccessKeyId=testid&Action=AddDomainRecord&DomainName=example.com&Format=JSON&RR=%40&SignatureMethod=HMAC-SHA1&SignatureNonce=fixed-nonce&SignatureVersion=1.0&TTL=600&Timestamp=2026-09-27T00%3A00%3A00Z&Type=TXT&Value=%E4%B8%AD%E6%96%87%20%2B%2A~%2F&Version=2015-01-09");
        let changed = signed_query("newid", "newsecret", "AddDomainRecord", &params,
            "2026-09-27T00:00:00Z", "fixed-nonce").unwrap();
        assert_ne!(query.split('&').next(), changed.split('&').next());
        assert!(changed.contains("AccessKeyId=newid&"));
        assert!(!changed.contains("testid"));
    }

    #[tokio::test]
    async fn separate_commands_reuse_connection_with_current_credentials() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/", listener.local_addr().unwrap());
        // 一个 socket 连续处理两次请求；新建第二个连接会使测试超时而非误判复用。
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut stream = BufReader::new(stream);
            let mut requests = Vec::new();
            for _ in 0..2 {
                let mut first = String::new();
                stream.read_line(&mut first).unwrap();
                requests.push(first);
                loop {
                    let mut line = String::new();
                    assert_ne!(stream.read_line(&mut line).unwrap(), 0);
                    if line == "\r\n" { break; }
                }
                stream.get_mut().write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: application/json\r\n\r\n{}").unwrap();
                stream.get_mut().flush().unwrap();
            }
            requests
        });
        let pool = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap();
        for id in ["old-id", "new-id"] {
            let mut rpc = AliyunRpc::new(&ProviderConfig {
                id: id.into(), key: "fixture-secret".into(), credential_ref: None,
            });
            rpc.client = pool.clone();
            rpc.call_at::<serde_json::Value>(&endpoint, "DescribeDomains", &[]).await.unwrap();
        }
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("POST /?Signature="));
        assert!(requests[0].contains("AccessKeyId=old-id&"));
        assert!(requests[1].contains("AccessKeyId=new-id&"));
        assert!(!requests[1].contains("old-id"));
    }

    #[test]
    fn errors_never_echo_remote_secrets() {
        let body = serde_json::json!({"Code": "https://example/?Signature=secret", "Message": "secret"});
        assert!(!service_error(403, &body).contains("secret"));
        assert!(service_error(403, &serde_json::json!({"Code": "SignatureDoesNotMatch"})).contains("签名校验失败"));
    }
}
