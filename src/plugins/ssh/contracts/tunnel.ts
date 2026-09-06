/** SSH 契约 · 隧道 */

/* ── 隧道 ── */

/** 隧道类型 */
export type TunnelType = 'local' | 'remote' | 'dynamic'

/** 隧道配置（随 profile 持久化在后端插件库） */
export interface TunnelConfig {
  /** 唯一 id（tun-<毫秒时间戳>） */
  id: string
  /** 所属服务器配置 id */
  profileId: string
  /** 显示名称 */
  name: string
  /** 隧道类型 */
  tunnelType: TunnelType
  /** 监听地址（local/dynamic 为本机侧；remote 为服务端侧） */
  listenHost: string
  /** 监听端口 */
  listenPort: number
  /** 目标主机（dynamic 为空） */
  targetHost?: string
  /** 目标端口（dynamic 为空） */
  targetPort?: number
  /** 连接建立后自动启动 */
  autoStart: boolean
}

/** 隧道运行状态 */
export type TunnelStatus = 'stopped' | 'starting' | 'running' | 'error'

/** 隧道运行时快照 */
export interface TunnelRuntime {
  /** 隧道配置（平铺） */
  id: string
  profileId: string
  name: string
  tunnelType: TunnelType
  listenHost: string
  listenPort: number
  targetHost?: string
  targetPort?: number
  autoStart: boolean
  /** 当前状态 */
  status: TunnelStatus
  /** 活动连接数 */
  connections: number
  /** 异常信息（status=error 时） */
  error?: string
}
