//! DNS 查询实现（hickory-resolver 0.26）
//! 支持任意记录类型 + 指定 DNS 服务器（IPv4/IPv6、system 走系统默认配置）；
//! PTR 查询自动把 IP 转反向域名（in-addr.arpa / ip6.arpa）。

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Instant;

use hickory_resolver::config::{NameServerConfig, ResolverConfig};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::proto::rr::{RData, RecordType};
use hickory_resolver::Resolver;

use super::models::{DnsAnswer, ServerQueryResult};

/// 系统默认解析配置的哨兵值（前端下拉「系统默认」）
const SYSTEM_SERVER: &str = "system";

/// 解析前端传入的记录类型字符串（大小写不敏感）
pub fn parse_record_type(rtype: &str) -> Result<RecordType, String> {
    let upper = rtype.trim().to_uppercase();
    match upper.as_str() {
        "A" => Ok(RecordType::A),
        "AAAA" => Ok(RecordType::AAAA),
        "CNAME" => Ok(RecordType::CNAME),
        "MX" => Ok(RecordType::MX),
        "TXT" => Ok(RecordType::TXT),
        "NS" => Ok(RecordType::NS),
        "SOA" => Ok(RecordType::SOA),
        "PTR" => Ok(RecordType::PTR),
        "CAA" => Ok(RecordType::CAA),
        "SRV" => Ok(RecordType::SRV),
        "ANY" => Ok(RecordType::ANY),
        _ => Err(format!("不支持的记录类型: {rtype}")),
    }
}

/// 服务器字符串转 IpAddr（system 返回 None 表示走系统配置）
fn server_ip(server: &str) -> Result<Option<IpAddr>, String> {
    if server.trim().eq_ignore_ascii_case(SYSTEM_SERVER) {
        return Ok(None);
    }
    // 支持「地址:端口」与裸地址；端口目前仅校验格式，固定走 UDP:53
    if let Ok(addr) = server.parse::<SocketAddr>() {
        return Ok(Some(addr.ip()));
    }
    if let Ok(ip) = server.parse::<IpAddr>() {
        return Ok(Some(ip));
    }
    Err(format!("服务器地址无效: {server}"))
}

/// PTR 查询时把 IPv4/IPv6 转反向域名（in-addr.arpa / ip6.arpa），非 IP 原样返回
fn reverse_name(domain: &str, rtype: RecordType) -> String {
    if rtype != RecordType::PTR {
        return domain.to_string();
    }
    if let Ok(ip) = domain.trim().parse::<Ipv4Addr>() {
        // 4.3.2.1 → 1.2.3.4.in-addr.arpa（字节序反转）
        let o = ip.octets();
        return format!("{}.{}.{}.{}.in-addr.arpa", o[3], o[2], o[1], o[0]);
    }
    if let Ok(ip) = domain.trim().parse::<Ipv6Addr>() {
        // 每字节拆高低 nibble，整体反转 + ip6.arpa（RFC 3596）
        let mut s = String::new();
        for b in ip.octets().iter().rev() {
            s.push_str(&format!("{:x}.{:x}.", b & 0x0f, b >> 4));
        }
        return format!("{s}ip6.arpa");
    }
    domain.to_string()
}

/// RData 转展示字符串（TXT 去引号拼接多段，其余用 Display）
fn rdata_to_string(data: &RData) -> String {
    if let RData::TXT(txt) = data {
        // TXT 的 Display 带引号，dig 风格不引号：逐段拼接
        return txt
            .txt_data
            .iter()
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect::<Vec<_>>()
            .join("");
    }
    data.to_string()
}

/// 按配置构建解析器（0.26 Builder 模式：config + Tokio 运行时提供者）
fn build_resolver(config: ResolverConfig) -> Result<Resolver<TokioRuntimeProvider>, String> {
    Resolver::builder_with_config(config, TokioRuntimeProvider::default())
        .build()
        .map_err(|e| e.to_string())
}

/// 查询单台服务器（失败返回 Err，由调用方包装成 ok=false 结果）
pub async fn query_server(
    domain: &str,
    rtype: RecordType,
    server: &str,
) -> Result<ServerQueryResult, String> {
    let start = Instant::now();
    // system → 读 OS 配置（/etc/resolv.conf / Windows 注册表）；否则构造单服务器配置
    let resolver = match server_ip(server)? {
        Some(ip) => {
            let config = ResolverConfig::from_parts(None, vec![], vec![NameServerConfig::udp(ip)]);
            build_resolver(config)?
        }
        None => {
            // 系统默认：builder_tokio 读取系统配置（Unix resolv.conf / Windows 注册表）
            hickory_resolver::TokioResolver::builder_tokio()
                .map_err(|e| e.to_string())?
                .build()
                .map_err(|e| e.to_string())?
        }
    };

    let lookup = resolver
        .lookup(&reverse_name(domain, rtype), rtype)
        .await
        .map_err(|e| e.to_string())?;

    // 应答记录展平为展示结构（0.26 Record 字段公开：name/ttl/data，record_type 为方法）
    let records: Vec<DnsAnswer> = lookup
        .answers()
        .iter()
        .map(|r| DnsAnswer {
            name: r.name.to_string(),
            record_type: r.record_type().to_string(),
            ttl: r.ttl,
            value: rdata_to_string(&r.data),
        })
        .collect();

    Ok(ServerQueryResult {
        server: server.to_string(),
        ok: true,
        error: None,
        elapsed_ms: start.elapsed().as_millis() as u64,
        records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 记录类型解析：合法类型与非法类型
    #[test]
    fn parse_record_type_ok() {
        assert_eq!(parse_record_type("a").unwrap(), RecordType::A);
        assert_eq!(parse_record_type("TXT").unwrap(), RecordType::TXT);
        assert!(parse_record_type("SPF").is_err());
    }

    /// PTR 反向域名转换（IPv4 / IPv6 / 非 IP 原样）
    #[test]
    fn reverse_name_works() {
        assert_eq!(
            reverse_name("1.2.3.4", RecordType::PTR),
            "4.3.2.1.in-addr.arpa"
        );
        assert_eq!(
            reverse_name("::1", RecordType::PTR),
            "1.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.ip6.arpa"
        );
        // 非 PTR 或非 IP：原样返回
        assert_eq!(reverse_name("example.com", RecordType::PTR), "example.com");
        assert_eq!(reverse_name("1.2.3.4", RecordType::A), "1.2.3.4");
    }

    /// 服务器地址解析：system / IPv4 / IPv6 / 非法
    #[test]
    fn server_ip_works() {
        assert!(server_ip("system").unwrap().is_none());
        assert_eq!(server_ip("8.8.8.8").unwrap().unwrap(), "8.8.8.8".parse::<IpAddr>().unwrap());
        assert_eq!(server_ip("2400:3200::1").unwrap().unwrap(), "2400:3200::1".parse::<IpAddr>().unwrap());
        assert!(server_ip("abc").is_err());
    }

    /// 真实网络查询（手动运行：cargo test -- --ignored dns_）
    /// 验证 hickory 链路 + 指定服务器 + TXT 去引号
    #[tokio::test]
    #[ignore]
    async fn query_real_network() {
        let r = query_server("example.com", RecordType::A, "223.5.5.5")
            .await
            .expect("查询应成功");
        assert!(r.ok);
        assert!(!r.records.is_empty(), "example.com 应有 A 记录");
        assert!(r.records.iter().all(|a| a.record_type == "A"));
        assert!(r.elapsed_ms < 10_000);

        // 多类型 + system 路径
        let r2 = query_server("example.com", RecordType::NS, "system")
            .await
            .expect("system 查询应成功");
        assert!(r2.records.iter().any(|a| a.record_type == "NS"));

        // TXT 记录值不带引号
        let r3 = query_server("example.com", RecordType::TXT, "223.5.5.5")
            .await
            .expect("TXT 查询应成功");
        for a in &r3.records {
            assert!(!a.value.starts_with('"'), "TXT 值不应带引号: {}", a.value);
        }
    }
}
