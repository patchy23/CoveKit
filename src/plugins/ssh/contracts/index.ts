//! SSH 契约 · 入口（重导出各域类型 + 命令清单 + 事件名 + 载荷/返回类型）

export * from './common'
export * from './compose'
export * from './terminal'
export * from './file'
export * from './monitor'
export * from './conn'
export * from './tunnel'

/* ── 命令清单 ── */

/** 命令清单（本插件命令的唯一出处） */
import type { Channel } from '@tauri-apps/api/core'
import type {
  ComposeAction,
  ComposeOutput,
  ComposeProject,
  ConnectStage,
  DockerContainer,
  EditSaveResult,
  FileListResult,
  FileTransferProgress,
  HostKeyVerifyRequest,
  KnownHostEntry,
  LogActionResult,
  MonitorData,
  ProcessDetail,
  ProcessInfo,
  RemoteFileContent,
  SshActionResult,
  SshBookmark,
  SshConnectOutcome,
  SshGroup,
  ServerConnection,
  ServerProfile,
  SshSystemInfoResult,
  SystemdService,
  TerminalClosed,
  TerminalData,
  TerminalLogError,
  TerminalSession,
  TunnelConfig,
  TunnelRuntime,
} from './types'

/** 一次性凭证覆盖（仅本次连接在内存中使用，不落任何存储） */
export interface CredentialOverride {
  password?: string
  privateKey?: string
  passphrase?: string
}

export const commands = {
  sshComposeList: 'ssh_compose_list',
  sshComposeAction: 'ssh_compose_action',
  sshComposeCreate: 'ssh_compose_create',
  sshComposeHome: 'ssh_compose_home',
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
  sshTerminalLogStart: 'ssh_terminal_log_start',
  sshTerminalLogStop: 'ssh_terminal_log_stop',
  sshTerminalLogOpenDir: 'ssh_terminal_log_open_dir',
  sshFileList: 'ssh_file_list',
  sshFileUpload: 'ssh_file_upload',
  sshFileDownload: 'ssh_file_download',
  sshFileDelete: 'ssh_file_delete',
  sshFileRename: 'ssh_file_rename',
  sshFileMkdir: 'ssh_file_mkdir',
  sshLocalList: 'ssh_local_list',
  sshLocalCreate: 'ssh_local_create',
  sshLocalDelete: 'ssh_local_delete',
  sshLocalRename: 'ssh_local_rename',
  sshFileCreate: 'ssh_file_create',
  sshFileChmod: 'ssh_file_chmod',
  sshBookmarkList: 'ssh_bookmark_list',
  sshBookmarkAdd: 'ssh_bookmark_add',
  sshBookmarkDelete: 'ssh_bookmark_delete',
  sshFileDownloadRecursive: 'ssh_file_download_recursive',
  sshTransferCancel: 'ssh_transfer_cancel',

  /* 远程编辑 */
  sshEditOpen: 'ssh_edit_open',
  sshEditSave: 'ssh_edit_save',

  /* 监控 + 服务 + 进程 + docker */
  sshMonitorGet: 'ssh_monitor_get',
  sshSystemInfoGet: 'ssh_system_info_get',
  sshServiceList: 'ssh_service_list',
  sshServiceAction: 'ssh_service_action',
  sshServiceLogs: 'ssh_service_logs',
  sshServiceConfig: 'ssh_service_config',
  sshProcessList: 'ssh_process_list',
  sshProcessDetail: 'ssh_process_detail',
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
    saveLocal?: boolean
  }
  ssh_profile_delete: { profileId: string }
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
  /** 会话日志：dir 传 null 表示用框架存储 logs 分区（<存储根>/logs/ssh） */
  ssh_terminal_log_start: { terminalId: string; dir: string | null }
  ssh_terminal_log_stop: { terminalId: string }
  ssh_terminal_log_open_dir: Record<string, never>

  /* 文件管理 */
  ssh_file_list: { connectionId: string; path: string }
  ssh_file_upload: { connectionId: string; localPath: string; remotePath: string }
  ssh_file_download: { connectionId: string; remotePath: string; localPath: string }
  ssh_file_delete: { connectionId: string; remotePath: string; recursive?: boolean }
  ssh_file_rename: { connectionId: string; oldPath: string; newPath: string }
  ssh_file_mkdir: { connectionId: string; path: string }
  ssh_local_list: { path: string }
  /** 本地新建文件/目录 */
  ssh_local_create: { path: string; isDir: boolean }
  /** 本地删除文件/目录（目录递归） */
  ssh_local_delete: { path: string; isDir: boolean }
  /** 本地重命名/移动 */
  ssh_local_rename: { oldPath: string; newPath: string }
  /** 远程新建空文件 */
  ssh_file_create: { connectionId: string; remotePath: string }
  /** 远程权限修改（acknowledgeRisk=系统目录内递归的风险确认） */
  ssh_file_chmod: {
    connectionId: string
    remotePath: string
    mode: number
    recursive?: boolean
    acknowledgeRisk?: boolean
  }
  /** 书签列表/新增/删除 */
  ssh_bookmark_list: { profileId: string }
  ssh_bookmark_add: { profileId: string; name: string; path: string }
  ssh_bookmark_delete: { id: string }
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
  ssh_system_info_get: { connectionId: string }

  /* 服务 */
  ssh_service_list: { connectionId: string; filter?: 'all' | 'active' | 'inactive' | 'failed' }
  ssh_service_action: {
    connectionId: string
    serviceName: string
    action: 'start' | 'stop' | 'restart'
  }
  ssh_service_logs: { connectionId: string; serviceName: string; lines?: number }
  ssh_service_config: { connectionId: string; serviceName: string }

  /* 进程 */
  ssh_process_list: { connectionId: string; sortBy?: 'cpu' | 'memory' | 'pid'; keyword?: string }
  ssh_process_detail: { connectionId: string; pid: number }
  ssh_process_kill: { connectionId: string; pid: number; force?: boolean }

  /* Docker */
  ssh_docker_list: { connectionId: string; composeProject?: string }
  ssh_compose_list: { connectionId: string }
  ssh_compose_action: {
    connectionId: string
    project: ComposeProject
    action: ComposeAction
    progress?: Channel<[boolean, number[]]>
    draftPath?: string
    draftContent?: string
  }
  ssh_compose_create: { connectionId: string; remotePath: string; content: string }
  ssh_compose_home: { connectionId: string }
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
  'ssh_connect' | 'ssh_profile_save' | 'ssh_docker_exec'
> & {
  ssh_connect: { payload: Payloads['ssh_connect'] }
  ssh_profile_save: { payload: Payloads['ssh_profile_save'] }
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
  ssh_terminal_log_start: LogActionResult
  ssh_terminal_log_stop: LogActionResult
  ssh_terminal_log_open_dir: void

  /* 文件管理 */
  ssh_file_list: FileListResult
  ssh_file_upload: FileTransferProgress
  ssh_file_download: FileTransferProgress
  ssh_file_delete: SshActionResult
  ssh_file_rename: SshActionResult
  ssh_file_mkdir: SshActionResult
  ssh_local_list: FileListResult
  ssh_local_create: SshActionResult
  ssh_local_delete: SshActionResult
  ssh_local_rename: SshActionResult
  ssh_file_create: SshActionResult
  ssh_file_chmod: SshActionResult
  ssh_bookmark_list: SshBookmark[]
  ssh_bookmark_add: SshBookmark
  ssh_bookmark_delete: void
  ssh_file_download_recursive: FileTransferProgress
  ssh_transfer_cancel: SshActionResult

  /* 远程编辑 */
  ssh_edit_open: RemoteFileContent
  ssh_edit_save: EditSaveResult

  /* 监控 */
  ssh_monitor_get: MonitorData
  ssh_system_info_get: SshSystemInfoResult

  /* 服务 */
  ssh_service_list: SystemdService[]
  ssh_service_action: SshActionResult
  ssh_service_logs: { ok: boolean; logs: string; error?: string }
  ssh_service_config: string

  /* 进程 */
  ssh_process_list: ProcessInfo[]
  ssh_process_detail: ProcessDetail
  ssh_process_kill: SshActionResult

  /* Docker */
  ssh_docker_list: DockerContainer[]
  ssh_compose_list: ComposeProject[]
  ssh_compose_action: ComposeOutput
  ssh_compose_create: void
  ssh_compose_home: string
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
  /** 会话日志写盘失败（磁盘满/权限/路径失效），前端据此停录并提示 */
  terminalLogError: 'ssh://terminal-log-error',
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
  'ssh://terminal-log-error': TerminalLogError
}
