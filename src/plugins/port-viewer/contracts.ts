/** 端口工具 DTO，与 Rust port_viewer/models.rs 同步。 */
export type Protocol = 'TCP' | 'UDP'
export type IpVersion = 'IPv4' | 'IPv6'

export interface PortEndpoint {
  protocol: Protocol
  family: IpVersion
  localAddress: string
  localPort: number
  /** UDP 和 TCP 监听没有远端。IPv6 地址包含非零 scope ID。 */
  remoteAddress: string | null
  remotePort: number | null
  pid: number
}

export interface PortEntry extends PortEndpoint {
  /** UDP 为 null，不能视为 TCP LISTEN。 */
  state: string | null
  /** FILETIME 十进制字符串；null 表示身份不可验证，不允许关闭。 */
  startedAt: string | null
  processName: string | null
  executablePath: string | null
  detailError: string | null
}

export interface PortSnapshot {
  entries: PortEntry[]
  /** 单个协议/地址族失败；不能当成该类没有占用。 */
  warnings: string[]
}

export const commands = {
  supported: 'port_viewer_supported',
  query: 'port_viewer_query',
  terminate: 'port_viewer_terminate',
} as const

export type Payloads = {
  port_viewer_supported: Record<string, never>
  port_viewer_query: Record<string, never>
  port_viewer_terminate: { endpoint: PortEndpoint; startedAt: string }
}
export type Results = {
  port_viewer_supported: boolean
  port_viewer_query: PortSnapshot
  port_viewer_terminate: void
}
