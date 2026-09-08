/** SSH 契约 · 通用类型（结果/认证/配置/分组/连接快照） */

export interface SshActionResult {
  ok: boolean

  error?: string
}

/** 认证方式 */

export type AuthMethod = 'password' | 'privateKey' | 'privateKeyWithPassphrase'

/** 服务器连接配置（凭证只存公共 Vault 的 credentialRef 引用，秘密永不入库/不落 profile） */

export interface ServerProfile {
  /** 唯一 id（profile-<毫秒时间戳>） */

  id: string

  /** 显示名称（如「生产服务器」） */

  name: string

  /** 主机地址 */

  host: string

  /** SSH 端口（默认 22） */

  port: number

  /** 登录用户名 */

  username: string

  /** 认证方式 */

  authMethod: AuthMethod

  /** 公共 Vault 凭证 id；未设置表示尚未保存凭证（连接时需一次性凭证） */

  credentialRef?: string

  /** 所属分组 id（未设置 = 未分组，固定沉底的虚拟组） */

  groupId?: string

  /** 备注 */

  remark?: string

  /** 最后连接时间（毫秒时间戳） */

  lastConnectedAt?: number
}

/** 服务器分组 */

export interface SshGroup {
  /** 唯一 id（group-<毫秒时间戳>） */

  id: string

  /** 分组名称 */

  name: string

  /** 排序权重（创建顺序自增） */

  sortOrder: number
}

/** 服务器连接状态 */

export type ConnectionStatus =
  'disconnected' | 'connecting' | 'connected' | 'error' | 'reconnecting'

/** 服务器连接快照（侧栏列表展示用） */

export interface ServerConnection {
  /** 关联的 ServerProfile id */

  profileId: string

  /** 会话唯一 id（conn-<毫秒时间戳>） */

  sessionId: string

  /** 当前状态 */

  status: ConnectionStatus

  /** 已连接的服务器地址 */

  host?: string

  /** 延迟毫秒（ping 或 SSH 握手耗时） */

  latencyMs?: number

  /** 错误信息（status=error 时） */

  error?: string

  /** 建立连接的毫秒时间戳 */

  connectedAt?: number
}

/* ── 终端 ── */
