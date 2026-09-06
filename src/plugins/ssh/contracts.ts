/**
 * SSH 工具插件 · IPC 契约（本插件私有，独立于框架与其它插件）
 * 与 src-tauri/src/plugins/ssh/models.rs 的 serde 结构同步。
 *
 * 契约分批：第 1 批 = 连接 + 终端 + 文件管理（ssh_connect ~ ssh_file_download）
 *          第 2 批 = 凭证 + 远程编辑（ssh_credential_* ~ ssh_edit_save）
 *          第 3 批 = 监控 + 服务 + 进程 + docker（ssh_monitor_* ~ ssh_docker_*）
 */

/* ── 通用 ── */

/** 操作结果（失败时 ok=false + error，不抛错给前端展示） */
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

/** 终端会话快照 */
export interface TerminalSession {
  /** 终端唯一 id（term-<毫秒时间戳>） */
  id: string
  /** 关联的连接 sessionId */
  connectionId: string
  /** 终端标题（默认 profile 名称） */
  title: string
  /** 当前行列数 */
  cols: number
  rows: number
  /** 是否活跃（前端正在展示） */
  active: boolean
}

/** 终端数据块（Rust 推送 → 前端渲染） */
export interface TerminalData {
  /** 关联的终端 id */
  terminalId: string
  /** 所属连接 id（空闲断开判定用：终端输出视为会话活跃） */
  connectionId: string
  /** 数据内容（原始字节，含 ANSI 转义序列） */
  data: string
  /** 毫秒时间戳 */
  time: number
}

/** 后端 PTY 通道已经关闭。 */
export interface TerminalClosed {
  terminalId: string
  /** 所属连接会话 id（前端据此触发断线自动重连） */
  connectionId: string
}

/* ── 文件管理 ── */

/** 远程文件条目 */
export interface RemoteFile {
  /** 文件名 */
  name: string
  /** 完整路径 */
  path: string
  /** 是否目录 */
  isDir: boolean
  /** 文件大小（字节，目录为 0） */
  size: number
  /** 修改时间（毫秒时间戳） */
  modifiedAt: number
  /** 权限字符串（如 drwxr-xr-x） */
  permissions: string
  /** 所有者 */
  owner: string
  /** 所属组 */
  group: string
}

/** 文件列表结果 */
export interface FileListResult {
  ok: boolean
  /** 当前路径 */
  path: string
  /** 父路径（根目录为 null） */
  parentPath?: string
  /** 文件列表 */
  files: RemoteFile[]
  error?: string
}

/** 文件传输进度 */
export interface FileTransferProgress {
  /** 本次传输唯一标识 */
  transferId: string
  /** 所属 SSH 连接会话 id */
  connectionId: string
  /** 本地路径 */
  localPath: string
  /** 远程路径 */
  remotePath: string
  /** 已传输字节 */
  transferred: number
  /** 总字节 */
  total: number
  /** 是否完成 */
  done: boolean
  /** 是否失败 */
  error?: string
}

/* ── 远程编辑 ── */

/** 远程文件内容 */
export interface RemoteFileContent {
  ok: boolean
  /** 文件路径 */
  path: string
  /** 文件内容（文本） */
  content: string
  /** 文件大小（字节） */
  size: number
  /** 编码（如 UTF-8 / GBK） */
  encoding: string
  /** 修改时间（毫秒；编辑器乐观锁基线） */
  modifiedAt?: number
  error?: string
}

/** 远程编辑保存结果（conflict=true 表示远端已被修改，未写入） */
export interface EditSaveResult {
  ok: boolean
  error?: string
  conflict?: boolean
  /** 冲突时远端当前 mtime（毫秒） */
  currentMtime?: number
}

/* ── 资源监控 ── */

/** 监控数据点 */
export interface MonitorData {
  /** CPU 使用率 0-100 */
  cpuPercent: number
  /** 内存使用率 0-100 */
  memoryPercent: number
  /** 内存已用（字节） */
  memoryUsed: number
  /** 内存总量（字节） */
  memoryTotal: number
  /** 磁盘使用率 0-100 */
  diskPercent: number
  /** 磁盘已用（字节） */
  diskUsed: number
  /** 磁盘总量（字节） */
  diskTotal: number
  /** 网络上行速率（字节/秒） */
  netUploadBps: number
  /** 网络下行速率（字节/秒） */
  netDownloadBps: number
  /** 采样时间（毫秒时间戳） */
  timestamp: number
}

/* ── 服务管理 ── */

/** systemd 服务条目 */
export interface SystemdService {
  /** 服务名（如 nginx.service） */
  name: string
  /** 描述 */
  description: string
  /** 加载状态（loaded / not-found / masked） */
  loadState: string
  /** 活动状态（active / inactive / failed） */
  activeState: string
  /** 子状态（running / dead / exited） */
  subState: string
  /** 是否开机自启 */
  enabled: boolean
}

/* ── 进程管理 ── */

/** 进程条目 */
export interface ProcessInfo {
  /** 进程 ID */
  pid: number
  /** 运行用户 */
  user: string
  /** CPU 使用率 0-100 */
  cpuPercent: number
  /** 内存使用率 0-100 */
  memoryPercent: number
  /** 内存占用（字节） */
  memoryBytes: number
  /** 启动时间（毫秒时间戳） */
  startedAt: number
  /** 完整命令行 */
  command: string
}

/* ── Docker ── */

/** Docker 容器条目 */
export interface DockerContainer {
  /** 容器 ID（短） */
  id: string
  /** 容器名 */
  name: string
  /** 镜像名 */
  image: string
  /** 状态（running / exited / paused） */
  status: string
  /** 本次运行持续时间 */
  uptime: string
  /** 端口映射（如 "80:80,443:443"） */
  ports: string
  /** 创建时间（毫秒时间戳） */
  createdAt: number
}

/** Docker 日志条目 */
export interface DockerLog {
  /** 容器 ID */
  containerId: string
  /** 日志内容 */
  content: string
  /** 时间戳 */
  time: number
}

/* ── 主机密钥校验与连接结果 ── */

/** 已知主机条目（patchyBox 私有 known_hosts） */
export interface KnownHostEntry {
  /** 主机地址 */
  host: string
  /** 端口 */
  port: number
  /** 公钥算法名（如 ssh-ed25519） */
  algorithm: string
  /** SHA256 指纹（SHA256:base64） */
  fingerprint: string
}

/** 主机密钥人工确认请求（后端在握手回调中推送，等待 ssh_host_key_respond） */
export interface HostKeyVerifyRequest {
  /** 本次连接尝试的唯一请求 id */
  requestId: string
  /** unknown = 首次连接；mismatch = 与已保存指纹不一致 */
  kind: 'unknown' | 'mismatch'
  host: string
  port: number
  algorithm: string
  fingerprint: string
  /** kind=mismatch 时已保存的指纹列表 */
  savedFingerprints: string[]
}

/** 连接阶段事件 */
export interface ConnectStage {
  /** 本次连接尝试的请求 id（对应连接结果中的 requestId） */
  requestId: string

  /** 所属服务器配置 id（前端据此把进度关联到工作区） */
  profileId: string  /** resolve / tcp / handshake / verify / auth / session */
  stage: string
  /** start / ok / fail */
  status: string
  message?: string
}

/** 稳定错误码 + 中文文案（detail 可复制，已脱敏） */
export interface SshConnectError {
  code: string
  message: string
  detail?: string
}

/** ssh_connect / ssh_reconnect 的结构化返回（业务失败不抛 IPC 异常） */
export interface SshConnectOutcome {
  ok: boolean
  connection?: ServerConnection
  requestId: string
  error?: SshConnectError
}

/** 一次性导入结果摘要 */
export interface SshImportResult {
  importedProfiles: number
  importedGroups: number
  migratedCredentials: number
}

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

/* ── 命令清单 ── */

/** 命令清单（本插件命令的唯一出处） */
export const commands = {
  /* 连接 */
  sshConnect: 'ssh_connect',
  sshDisconnect: 'ssh_disconnect',
  sshReconnect: 'ssh_reconnect',
  sshConnections: 'ssh_connections',
  sshHostKeyRespond: 'ssh_host_key_respond',
  sshKnownHostList: 'ssh_known_host_list',
  sshKnownHostDelete: 'ssh_known_host_delete',

  /* 服务器配置 + 分组 */
  sshProfileList: 'ssh_profile_list',
  sshProfileSave: 'ssh_profile_save',
  sshProfileDelete: 'ssh_profile_delete',
  sshProfileImport: 'ssh_profile_import',
  sshGroupList: 'ssh_group_list',
  sshGroupSave: 'ssh_group_save',
  sshGroupDelete: 'ssh_group_delete',

  /* 隧道 */
  sshTunnelList: 'ssh_tunnel_list',
  sshTunnelSave: 'ssh_tunnel_save',
  sshTunnelStart: 'ssh_tunnel_start',
  sshTunnelStop: 'ssh_tunnel_stop',
  sshTunnels: 'ssh_tunnels',
  sshTunnelDelete: 'ssh_tunnel_delete',

  /* 终端 */
  sshTerminalOpen: 'ssh_terminal_open',
  sshTerminalWrite: 'ssh_terminal_write',
  sshTerminalResize: 'ssh_terminal_resize',
  sshTerminalClose: 'ssh_terminal_close',
  sshTerminalList: 'ssh_terminal_list',
  sshFileList: 'ssh_file_list',
  sshFileUpload: 'ssh_file_upload',
  sshFileDownload: 'ssh_file_download',
  sshFileDelete: 'ssh_file_delete',
  sshFileRename: 'ssh_file_rename',
  sshFileMkdir: 'ssh_file_mkdir',
  sshLocalList: 'ssh_local_list',
  sshFileDownloadRecursive: 'ssh_file_download_recursive',
  sshTransferCancel: 'ssh_transfer_cancel',

  /* 远程编辑 */
  sshEditOpen: 'ssh_edit_open',
  sshEditSave: 'ssh_edit_save',

  /* 监控 + 服务 + 进程 + docker */
  sshMonitorGet: 'ssh_monitor_get',
  sshServiceList: 'ssh_service_list',
  sshServiceAction: 'ssh_service_action',
  sshServiceLogs: 'ssh_service_logs',
  sshProcessList: 'ssh_process_list',
  sshProcessKill: 'ssh_process_kill',
  sshDockerList: 'ssh_docker_list',
  sshDockerAction: 'ssh_docker_action',
  sshDockerLogs: 'ssh_docker_logs',
  sshDockerExec: 'ssh_docker_exec',
} as const

/* ── 命令入参 ── */

/** 一次性凭证覆盖（仅本次连接在内存中使用，不落任何存储） */
export interface CredentialOverride {
  password?: string
  privateKey?: string
  passphrase?: string
}

export type Payloads = {
  /* 连接 */
  ssh_connect: { profileId: string; overrides?: CredentialOverride }
  ssh_disconnect: { sessionId: string }
  ssh_reconnect: { sessionId: string; overrides?: CredentialOverride }
  ssh_connections: Record<string, never>
  ssh_host_key_respond: {
    requestId: string
    decision: 'trustOnce' | 'trustSave' | 'cancel' | 'replace'
  }
  ssh_known_host_list: Record<string, never>
  ssh_known_host_delete: { host: string; port: number; fingerprint?: string }

  /* 服务器配置 + 分组 */
  ssh_profile_list: Record<string, never>
  ssh_profile_save: {
    profile: ServerProfile
    password?: string
    privateKey?: string
    passphrase?: string
    saveCredential: boolean
  }
  ssh_profile_delete: { profileId: string }
  ssh_profile_import: { profiles: ServerProfile[]; groups: SshGroup[] }
  ssh_group_list: Record<string, never>
  ssh_group_save: { group: SshGroup }
  ssh_group_delete: { groupId: string }

  /* 隧道 */
  ssh_tunnel_list: { profileId: string }
  ssh_tunnel_save: { config: TunnelConfig }
  ssh_tunnel_start: { connectionId: string; tunnelId: string }
  ssh_tunnel_stop: { tunnelId: string }
  ssh_tunnels: { connectionId: string }
  ssh_tunnel_delete: { tunnelId: string }

  /* 终端 */
  ssh_terminal_open: { connectionId: string; cols: number; rows: number }
  ssh_terminal_write: { terminalId: string; data: string }
  ssh_terminal_resize: { terminalId: string; cols: number; rows: number }
  ssh_terminal_close: { terminalId: string }
  ssh_terminal_list: { connectionId: string }

  /* 文件管理 */
  ssh_file_list: { connectionId: string; path: string }
  ssh_file_upload: { connectionId: string; localPath: string; remotePath: string }
  ssh_file_download: { connectionId: string; remotePath: string; localPath: string }
  ssh_file_delete: { connectionId: string; remotePath: string; recursive?: boolean }
  ssh_file_rename: { connectionId: string; oldPath: string; newPath: string }
  ssh_file_mkdir: { connectionId: string; path: string }
  ssh_local_list: { path: string }
  ssh_file_download_recursive: {
    connectionId: string
    remotePath: string
    localPath: string
    overwrite?: boolean
  }
  ssh_transfer_cancel: { transferId: string }

  /* 远程编辑 */
  ssh_edit_open: { connectionId: string; remotePath: string }
  ssh_edit_save: {
    connectionId: string
    remotePath: string
    content: string
    expectedMtime?: number
  }

  /* 监控 */
  ssh_monitor_get: { connectionId: string }

  /* 服务 */
  ssh_service_list: { connectionId: string; filter?: 'all' | 'active' | 'inactive' | 'failed' }
  ssh_service_action: {
    connectionId: string
    serviceName: string
    action: 'start' | 'stop' | 'restart'
  }
  ssh_service_logs: { connectionId: string; serviceName: string; lines?: number }

  /* 进程 */
  ssh_process_list: { connectionId: string; sortBy?: 'cpu' | 'memory' | 'pid'; keyword?: string }
  ssh_process_kill: { connectionId: string; pid: number; force?: boolean }

  /* Docker */
  ssh_docker_list: { connectionId: string }
  ssh_docker_action: {
    connectionId: string
    containerId: string
    action: 'start' | 'stop' | 'restart' | 'remove'
  }
  ssh_docker_logs: { connectionId: string; containerId: string; lines?: number }
  ssh_docker_exec: {
    connectionId: string
    containerId: string
    shell: '/bin/sh' | '/bin/bash'
    cols: number
    rows: number
  }
}

/** 前端 invoke 的真实顶层参数；Rust payload 结构体命令在此统一声明包裹层。 */
export type InvokePayloads = Omit<
  Payloads,
  | 'ssh_connect'
  | 'ssh_reconnect'
  | 'ssh_profile_save'
  | 'ssh_profile_import'
  | 'ssh_docker_exec'
> & {
  ssh_connect: { payload: Payloads['ssh_connect'] }
  ssh_reconnect: { sessionId: string; payload: Omit<Payloads['ssh_reconnect'], 'sessionId'> }
  ssh_profile_save: { payload: Payloads['ssh_profile_save'] }
  ssh_profile_import: { payload: Payloads['ssh_profile_import'] }
  ssh_docker_exec: { payload: Payloads['ssh_docker_exec'] }
}

/* ── 命令返回 ── */

export type Results = {
  /* 连接 */
  ssh_connect: SshConnectOutcome
  ssh_disconnect: SshActionResult
  ssh_reconnect: SshConnectOutcome
  ssh_connections: ServerConnection[]
  ssh_host_key_respond: SshActionResult
  ssh_known_host_list: KnownHostEntry[]
  ssh_known_host_delete: SshActionResult

  /* 服务器配置 + 分组 */
  ssh_profile_list: ServerProfile[]
  ssh_profile_save: ServerProfile
  ssh_profile_delete: void
  ssh_profile_import: SshImportResult
  ssh_group_list: SshGroup[]
  ssh_group_save: void
  ssh_group_delete: void

  /* 隧道 */
  ssh_tunnel_list: TunnelConfig[]
  ssh_tunnel_save: TunnelConfig
  ssh_tunnel_start: TunnelRuntime
  ssh_tunnel_stop: TunnelRuntime
  ssh_tunnels: TunnelRuntime[]
  ssh_tunnel_delete: void

  /* 终端 */
  ssh_terminal_open: TerminalSession
  ssh_terminal_write: SshActionResult
  ssh_terminal_resize: SshActionResult
  ssh_terminal_close: SshActionResult
  ssh_terminal_list: TerminalSession[]

  /* 文件管理 */
  ssh_file_list: FileListResult
  ssh_file_upload: FileTransferProgress
  ssh_file_download: FileTransferProgress
  ssh_file_delete: SshActionResult
  ssh_file_rename: SshActionResult
  ssh_file_mkdir: SshActionResult
  ssh_local_list: FileListResult
  ssh_file_download_recursive: FileTransferProgress
  ssh_transfer_cancel: SshActionResult

  /* 远程编辑 */
  ssh_edit_open: RemoteFileContent
  ssh_edit_save: EditSaveResult

  /* 监控 */
  ssh_monitor_get: MonitorData

  /* 服务 */
  ssh_service_list: SystemdService[]
  ssh_service_action: SshActionResult
  ssh_service_logs: { ok: boolean; logs: string; error?: string }

  /* 进程 */
  ssh_process_list: ProcessInfo[]
  ssh_process_kill: SshActionResult

  /* Docker */
  ssh_docker_list: DockerContainer[]
  ssh_docker_action: SshActionResult
  ssh_docker_logs: { ok: boolean; logs: string; error?: string }
  ssh_docker_exec: TerminalSession
}

/* ── 事件通道（Rust → 前端推送，与命令契约并列）──
 * 交互式终端数据、文件传输进度、连接状态变化均为异步推送，
 * 前端用 `listen(sshEvents.xxx, handler)` 订阅，不轮询。
 */

/** Tauri 事件名（全局唯一，`ssh://` 前缀隔离命名空间） */
export const sshEvents = {
  /** 终端输出推送（含 ANSI 转义序列，xterm.js 直接渲染） */
  terminalData: 'ssh://terminal-data',
  terminalClosed: 'ssh://terminal-closed',
  /** 上传/下载进度推送 */
  transferProgress: 'ssh://transfer-progress',
  /** 连接/断开/重连状态变化推送 */
  connectionStatus: 'ssh://connection-status',
  /** 主机密钥人工确认请求（首连/指纹变更），等待 ssh_host_key_respond 应答 */
  hostKeyVerify: 'ssh://host-key-verify',
  /** 分阶段连接进度（resolve/tcp/handshake/verify/auth/session） */
  connectStage: 'ssh://connect-stage',
  /** 隧道状态变化（启停/错误/连接数） */
  tunnelStatus: 'ssh://tunnel-status',
} as const

/** 事件负载类型（与命令出参类型同源） */
export type SshEventPayloads = {
  'ssh://terminal-data': TerminalData
  'ssh://terminal-closed': TerminalClosed
  'ssh://transfer-progress': FileTransferProgress
  'ssh://connection-status': ServerConnection
  'ssh://host-key-verify': HostKeyVerifyRequest
  'ssh://connect-stage': ConnectStage
  'ssh://tunnel-status': TunnelRuntime
}
