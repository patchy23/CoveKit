//! 端口查询传输契约，与前端 port-viewer/contracts.ts 同步。

use serde::{Deserialize, Serialize};

/// 传输层协议；TCP 状态与 UDP 绑定分别展示。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Protocol {
    Tcp,
    Udp,
}

/// 地址族，关闭时必须匹配，不把 IPv4 与 IPv6 同端口合并。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpVersion {
    #[serde(rename = "IPv4")]
    V4,
    #[serde(rename = "IPv6")]
    V6,
}

/// 系统端点身份；端口号均为主机字节序，IPv6 地址保留非零 scope ID。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortEndpoint {
    pub(crate) protocol: Protocol,
    pub(crate) family: IpVersion,
    pub(crate) local_address: String,
    pub(crate) local_port: u16,
    /// UDP 与 TCP 监听记录没有远端，序列化为 null。
    pub(crate) remote_address: Option<String>,
    pub(crate) remote_port: Option<u16>,
    /// 系统连接可能返回 0，不能据此关闭进程。
    pub(crate) pid: u32,
}

/// 一行端口与进程快照，基本端点在详情权限不足时仍保留。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortEntry {
    #[serde(flatten)]
    pub(crate) endpoint: PortEndpoint,
    /// TCP 状态名；UDP 为 null，不虚构 LISTEN 状态。
    pub(crate) state: Option<String>,
    /// FILETIME 十进制字符串；无法核对身份时为 null，禁止关闭。
    pub(crate) started_at: Option<String>,
    pub(crate) process_name: Option<String>,
    pub(crate) executable_path: Option<String>,
    pub(crate) detail_error: Option<String>,
}

/// 四类系统表合并结果，单表失败以 warnings 展示，不当成没有占用。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortSnapshot {
    pub(crate) entries: Vec<PortEntry>,
    pub(crate) warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn endpoint_and_snapshot_match_the_frontend_wire_contract() {
        let wire = json!({
            "protocol": "UDP", "family": "IPv6", "localAddress": "fe80::1%7",
            "localPort": 8080, "remoteAddress": null, "remotePort": null, "pid": 42
        });
        let endpoint: PortEndpoint = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(endpoint.protocol, Protocol::Udp);
        assert_eq!(endpoint.family, IpVersion::V6);
        let entry = PortEntry {
            endpoint,
            state: None,
            started_at: Some("134029000000000001".into()),
            process_name: Some("server.exe".into()),
            executable_path: None,
            detail_error: None,
        };
        let value = serde_json::to_value(PortSnapshot {
            entries: vec![entry],
            warnings: vec![],
        })
        .unwrap();
        let row = &value["entries"][0];
        assert!(row.get("endpoint").is_none());
        assert_eq!(row["localAddress"], wire["localAddress"]);
        assert_eq!(row["protocol"], "UDP");
        assert_eq!(row["family"], "IPv6");
        assert_eq!(row["localPort"], 8080);
        assert_eq!(row["startedAt"], "134029000000000001");
        assert_eq!(row["remoteAddress"], serde_json::Value::Null);
        assert_eq!(row["state"], serde_json::Value::Null);
        assert_eq!(row["executablePath"], serde_json::Value::Null);
        assert_eq!(value["warnings"], json!([]));
    }
}
