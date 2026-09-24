//! IP Helper 端点快照与关闭前复核；缓冲和重试有界，不解析命令行文本。

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::{offset_of, size_of};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};

use super::models::{IpVersion, PortEndpoint, PortEntry, PortSnapshot, Protocol};
use crate::framework::windows_process;
use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID,
    MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, MIB_UDP6ROW_OWNER_PID, MIB_UDP6TABLE_OWNER_PID,
    MIB_UDPROW_OWNER_PID, MIB_UDPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 65536;
static OPERATION_RUNNING: AtomicBool = AtomicBool::new(false);

/// 防止重开工具后堆积快照或同时关闭多个目标。
pub(super) struct OperationPermit;
impl OperationPermit {
    /// 占用端口工具的单次系统操作名额，结束时自动释放。
    pub(super) fn acquire() -> Result<Self, String> {
        OPERATION_RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "已有端口操作正在进行，请稍后重试")?;
        Ok(Self)
    }
}
impl Drop for OperationPermit {
    fn drop(&mut self) {
        OPERATION_RUNNING.store(false, Ordering::Release);
    }
}

struct TableBuffer {
    words: Vec<u32>,
    bytes: usize,
}
struct SocketRow {
    endpoint: PortEndpoint,
    state: Option<String>,
}

fn fetch_table(mut fetch: impl FnMut(*mut c_void, &mut u32) -> u32) -> Result<TableBuffer, String> {
    let mut words: Vec<u32> = Vec::new();
    for _ in 0..5 {
        let capacity = words.len() * size_of::<u32>();
        let mut size = u32::try_from(capacity).map_err(|_| "端口表过大")?;
        let ptr = if words.is_empty() {
            null_mut()
        } else {
            words.as_mut_ptr().cast()
        };
        let status = fetch(ptr, &mut size);
        let required = usize::try_from(size).map_err(|_| "端口表长度无效")?;
        if status == ERROR_SUCCESS {
            if required < 4 || required > capacity {
                return Err("系统端口表长度无效".into());
            }
            return Ok(TableBuffer {
                words,
                bytes: required,
            });
        }
        if status != ERROR_INSUFFICIENT_BUFFER {
            return Err(format!("读取系统端口表失败，Windows 错误码 {status}"));
        }
        if !(4..=MAX_BYTES).contains(&required) {
            return Err("系统端口表过大或长度无效".into());
        }
        words.resize(required.div_ceil(size_of::<u32>()), 0);
    }
    Err("系统端口表变化过于频繁，请稍后重试".into())
}

// 私有约束只允许这四种 SDK 行：所有位模式都有效，无指针、引用或受限枚举。
trait OwnerPidRow: Copy {}
impl OwnerPidRow for MIB_TCPROW_OWNER_PID {}
impl OwnerPidRow for MIB_TCP6ROW_OWNER_PID {}
impl OwnerPidRow for MIB_UDPROW_OWNER_PID {}
impl OwnerPidRow for MIB_UDP6ROW_OWNER_PID {}

fn rows<T: OwnerPidRow>(table: &TableBuffer, offset: usize) -> Result<Vec<T>, String> {
    let count = usize::try_from(*table.words.first().ok_or("缺少端口表表头")?)
        .map_err(|_| "端口条目数无效")?;
    let end = count
        .checked_mul(size_of::<T>())
        .and_then(|bytes| offset.checked_add(bytes))
        .ok_or("端口表长度溢出")?;
    if count > MAX_ENTRIES || end > table.bytes || table.bytes > table.words.len() * 4 {
        return Err("系统端口表条目越界或数量过多".into());
    }
    let mut output = Vec::with_capacity(count);
    for index in 0..count {
        // SAFETY: T 仅取四种全整数 SDK 行；上述检查保证整行位于已初始化缓冲内，read_unaligned 不要求行对齐。
        let row = unsafe {
            table
                .words
                .as_ptr()
                .cast::<u8>()
                .add(offset + index * size_of::<T>())
                .cast::<T>()
                .read_unaligned()
        };
        output.push(row);
    }
    Ok(output)
}

fn port(value: u32) -> u16 {
    let bytes = value.to_ne_bytes();
    u16::from_be_bytes([bytes[0], bytes[1]])
}
fn ipv4(value: u32) -> String {
    Ipv4Addr::from(value.to_ne_bytes()).to_string()
}
fn ipv6(value: [u8; 16], scope: u32) -> String {
    let address = Ipv6Addr::from(value);
    if scope == 0 {
        address.to_string()
    } else {
        format!("{address}%{scope}")
    }
}
fn state(value: u32) -> String {
    match value {
        1 => "CLOSED".into(),
        2 => "LISTEN".into(),
        3 => "SYN_SENT".into(),
        4 => "SYN_RECEIVED".into(),
        5 => "ESTABLISHED".into(),
        6 => "FIN_WAIT_1".into(),
        7 => "FIN_WAIT_2".into(),
        8 => "CLOSE_WAIT".into(),
        9 => "CLOSING".into(),
        10 => "LAST_ACK".into(),
        11 => "TIME_WAIT".into(),
        12 => "DELETE_TCB".into(),
        _ => format!("UNKNOWN({value})"),
    }
}

fn read_endpoints(protocol: Protocol, family: IpVersion) -> Result<Vec<SocketRow>, String> {
    let af = u32::from(if family == IpVersion::V4 {
        AF_INET
    } else {
        AF_INET6
    });
    let table = fetch_table(|ptr, size| {
        // SAFETY: fetch_table 提供按 u32 对齐、长度为 size 的可写缓冲，空缓冲传 null；仅请求已知地址族与 OWNER_PID 表。
        unsafe {
            match protocol {
                Protocol::Tcp => GetExtendedTcpTable(ptr, size, 0, af, TCP_TABLE_OWNER_PID_ALL, 0),
                Protocol::Udp => GetExtendedUdpTable(ptr, size, 0, af, UDP_TABLE_OWNER_PID, 0),
            }
        }
    })?;
    let mut result = Vec::new();
    match (protocol, family) {
        (Protocol::Tcp, IpVersion::V4) => {
            for row in
                rows::<MIB_TCPROW_OWNER_PID>(&table, offset_of!(MIB_TCPTABLE_OWNER_PID, table))?
            {
                result.push(SocketRow {
                    endpoint: PortEndpoint {
                        protocol,
                        family,
                        local_address: ipv4(row.dwLocalAddr),
                        local_port: port(row.dwLocalPort),
                        remote_address: (row.dwState != 2).then(|| ipv4(row.dwRemoteAddr)),
                        remote_port: (row.dwState != 2).then(|| port(row.dwRemotePort)),
                        pid: row.dwOwningPid,
                    },
                    state: Some(state(row.dwState)),
                });
            }
        }
        (Protocol::Tcp, IpVersion::V6) => {
            for row in
                rows::<MIB_TCP6ROW_OWNER_PID>(&table, offset_of!(MIB_TCP6TABLE_OWNER_PID, table))?
            {
                result.push(SocketRow {
                    endpoint: PortEndpoint {
                        protocol,
                        family,
                        local_address: ipv6(row.ucLocalAddr, row.dwLocalScopeId),
                        local_port: port(row.dwLocalPort),
                        remote_address: (row.dwState != 2)
                            .then(|| ipv6(row.ucRemoteAddr, row.dwRemoteScopeId)),
                        remote_port: (row.dwState != 2).then(|| port(row.dwRemotePort)),
                        pid: row.dwOwningPid,
                    },
                    state: Some(state(row.dwState)),
                });
            }
        }
        (Protocol::Udp, IpVersion::V4) => {
            for row in
                rows::<MIB_UDPROW_OWNER_PID>(&table, offset_of!(MIB_UDPTABLE_OWNER_PID, table))?
            {
                result.push(SocketRow {
                    endpoint: PortEndpoint {
                        protocol,
                        family,
                        local_address: ipv4(row.dwLocalAddr),
                        local_port: port(row.dwLocalPort),
                        remote_address: None,
                        remote_port: None,
                        pid: row.dwOwningPid,
                    },
                    state: None,
                });
            }
        }
        (Protocol::Udp, IpVersion::V6) => {
            for row in
                rows::<MIB_UDP6ROW_OWNER_PID>(&table, offset_of!(MIB_UDP6TABLE_OWNER_PID, table))?
            {
                result.push(SocketRow {
                    endpoint: PortEndpoint {
                        protocol,
                        family,
                        local_address: ipv6(row.ucLocalAddr, row.dwLocalScopeId),
                        local_port: port(row.dwLocalPort),
                        remote_address: None,
                        remote_port: None,
                        pid: row.dwOwningPid,
                    },
                    state: None,
                });
            }
        }
    }
    Ok(result)
}

/// 读取四类端点表；每个 PID 只补全一次详情，不持有跨请求缓存。
pub(super) fn query() -> Result<PortSnapshot, String> {
    let observed_at = windows_process::snapshot_time();
    let mut sockets = Vec::new();
    let mut warnings = Vec::new();
    for protocol in [Protocol::Tcp, Protocol::Udp] {
        for family in [IpVersion::V4, IpVersion::V6] {
            match read_endpoints(protocol, family) {
                Ok(rows) => sockets.extend(rows),
                Err(error) => {
                    let protocol = match protocol {
                        Protocol::Tcp => "TCP",
                        Protocol::Udp => "UDP",
                    };
                    let family = match family {
                        IpVersion::V4 => "IPv4",
                        IpVersion::V6 => "IPv6",
                    };
                    warnings.push(format!("{protocol} {family}：{error}"));
                }
            }
        }
    }
    if warnings.len() == 4 {
        return Err(warnings.join("；"));
    }
    if sockets.len() > MAX_ENTRIES {
        return Err("端口条目过多，请稍后重试".into());
    }
    let mut cache = HashMap::new();
    let mut entries = Vec::with_capacity(sockets.len());
    for socket in sockets {
        let detail = if socket.endpoint.pid == 0 {
            None
        } else {
            Some(cache.entry(socket.endpoint.pid).or_insert_with(|| {
                let info = windows_process::inspect(socket.endpoint.pid, None)?;
                if info.started_at > observed_at {
                    return Err("进程在查询期间发生变化，请刷新".to_string());
                }
                Ok(info)
            }))
        };
        let (started_at, process_name, executable_path, detail_error) = match detail {
            Some(Ok(info)) => (
                Some(info.started_at.to_string()),
                Some(info.process_name.clone()),
                Some(info.executable_path.clone()),
                None,
            ),
            Some(Err(error)) => (None, None, None, Some(error.clone())),
            None => (None, None, None, None),
        };
        entries.push(PortEntry {
            endpoint: socket.endpoint,
            state: socket.state,
            started_at,
            process_name,
            executable_path,
            detail_error,
        });
    }
    entries.sort_by(|left, right| {
        left.endpoint
            .local_port
            .cmp(&right.endpoint.local_port)
            .then(left.endpoint.pid.cmp(&right.endpoint.pid))
            .then(
                left.endpoint
                    .local_address
                    .cmp(&right.endpoint.local_address),
            )
    });
    Ok(PortSnapshot { entries, warnings })
}

/// 后端重新核对完整端点身份；共享平台能力保持同一进程句柄直到关闭完成。
pub(super) fn terminate(endpoint: &PortEndpoint, started_at: &str) -> Result<(), String> {
    windows_process::terminate(endpoint.pid, started_at, || {
        let rows = read_endpoints(endpoint.protocol, endpoint.family)?;
        if rows.iter().any(|row| row.endpoint == *endpoint) {
            Ok(())
        } else {
            Err("该进程已不再使用指定端口，请刷新查询".into())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_network_order_and_ipv6_scope() {
        assert_eq!(port(u32::from_ne_bytes([0x1f, 0x90, 0, 0])), 8080);
        assert_eq!(ipv4(u32::from_ne_bytes([127, 0, 0, 1])), "127.0.0.1");
        assert_eq!(ipv6(Ipv6Addr::LOCALHOST.octets(), 0), "::1");
        assert_eq!(
            ipv6("fe80::1".parse::<Ipv6Addr>().unwrap().octets(), 7),
            "fe80::1%7"
        );
    }

    #[test]
    fn bounds_table_rows_and_retries() {
        let empty = TableBuffer {
            words: vec![0],
            bytes: 4,
        };
        assert!(rows::<MIB_TCPROW_OWNER_PID>(&empty, 4).unwrap().is_empty());
        let short = TableBuffer {
            words: vec![1, 0],
            bytes: 8,
        };
        assert!(rows::<MIB_TCPROW_OWNER_PID>(&short, 4).is_err());
        let mut calls = 0;
        assert!(fetch_table(|_, size| {
            calls += 1;
            *size = 8;
            ERROR_INSUFFICIENT_BUFFER
        })
        .is_err());
        assert_eq!(calls, 5);
        assert!(fetch_table(|_, size| {
            *size = u32::MAX;
            ERROR_INSUFFICIENT_BUFFER
        })
        .is_err());
        assert!(fetch_table(|_, _| 5).is_err());
    }

    #[test]
    fn decodes_owner_pid_rows_without_losing_same_port_records() {
        let address = u32::from_ne_bytes([127, 0, 0, 1]);
        let port_word = u32::from_ne_bytes([0x1f, 0x90, 0, 0]);
        let words = vec![
            2, 2, address, port_word, 0, 0, 42, 5, address, port_word, address, port_word, 43,
        ];
        let table = TableBuffer {
            bytes: words.len() * 4,
            words,
        };
        let decoded =
            rows::<MIB_TCPROW_OWNER_PID>(&table, offset_of!(MIB_TCPTABLE_OWNER_PID, table))
                .unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].dwOwningPid, 42);
        assert_eq!(decoded[1].dwOwningPid, 43);
        assert_eq!(port(decoded[1].dwRemotePort), 8080);
        assert_eq!(state(decoded[0].dwState), "LISTEN");
        assert_eq!(state(decoded[1].dwState), "ESTABLISHED");

        let words = vec![1, address, port_word, 42];
        let table = TableBuffer {
            bytes: words.len() * 4,
            words,
        };
        let decoded =
            rows::<MIB_UDPROW_OWNER_PID>(&table, offset_of!(MIB_UDPTABLE_OWNER_PID, table))
                .unwrap();
        assert_eq!(decoded[0].dwOwningPid, 42);
        assert_eq!(ipv4(decoded[0].dwLocalAddr), "127.0.0.1");
    }

    #[test]
    fn decodes_ipv6_scope_and_remote_fields() {
        let port_word = u32::from_ne_bytes([0x1f, 0x90, 0, 0]);
        let last = u32::from_ne_bytes([0, 0, 0, 1]);
        let words = vec![
            1, 0, 0, 0, last, 7, port_word, 0, 0, 0, last, 9, port_word, 5, 42,
        ];
        let table = TableBuffer {
            bytes: words.len() * 4,
            words,
        };
        let decoded =
            rows::<MIB_TCP6ROW_OWNER_PID>(&table, offset_of!(MIB_TCP6TABLE_OWNER_PID, table))
                .unwrap();
        let row = &decoded[0];
        assert_eq!(ipv6(row.ucLocalAddr, row.dwLocalScopeId), "::1%7");
        assert_eq!(ipv6(row.ucRemoteAddr, row.dwRemoteScopeId), "::1%9");
        assert_eq!(port(row.dwLocalPort), 8080);
        assert_eq!(row.dwOwningPid, 42);
        assert_eq!(row.dwState, 5);

        let words = vec![1, 0, 0, 0, last, 7, port_word, 42];
        let table = TableBuffer {
            bytes: words.len() * 4,
            words,
        };
        let decoded =
            rows::<MIB_UDP6ROW_OWNER_PID>(&table, offset_of!(MIB_UDP6TABLE_OWNER_PID, table))
                .unwrap();
        assert_eq!(
            ipv6(decoded[0].ucLocalAddr, decoded[0].dwLocalScopeId),
            "::1%7"
        );
        assert_eq!(decoded[0].dwOwningPid, 42);
    }

    #[test]
    fn reallocates_when_a_table_grows_between_calls() {
        let mut calls = 0;
        let table = fetch_table(|ptr, size| {
            calls += 1;
            match calls {
                1 => {
                    assert!(ptr.is_null());
                    *size = 4;
                    ERROR_INSUFFICIENT_BUFFER
                }
                2 => {
                    assert_eq!(*size, 4);
                    *size = 8;
                    ERROR_INSUFFICIENT_BUFFER
                }
                _ => {
                    assert_eq!(*size, 8);
                    ERROR_SUCCESS
                }
            }
        })
        .unwrap();
        assert_eq!(calls, 3);
        assert!(rows::<MIB_TCPROW_OWNER_PID>(&table, 4).unwrap().is_empty());
    }

    // Windows 测试：只绑定系统分配的本机临时端口，句柄随测试退出释放。
    #[test]
    fn finds_isolated_tcp_listener_and_udp_binding() {
        let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let udp = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let pid = std::process::id();
        let rows = read_endpoints(Protocol::Tcp, IpVersion::V4).unwrap();
        assert!(rows.iter().any(|row| row.endpoint.pid == pid
            && row.endpoint.local_port == tcp.local_addr().unwrap().port()
            && row.state.as_deref() == Some("LISTEN")));
        let rows = read_endpoints(Protocol::Udp, IpVersion::V4).unwrap();
        assert!(rows.iter().any(|row| row.endpoint.pid == pid
            && row.endpoint.local_port == udp.local_addr().unwrap().port()
            && row.state.is_none()));
    }
}
