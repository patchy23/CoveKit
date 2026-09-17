/**
 * SSH 工具插件 · IPC 封装（本插件命令与事件，独立于框架）
 * 命令命名与 Rust 侧 serde（camelCase）一致；事件经 listen 订阅。
 */
import { invokeCommand } from '@/core/ipc/ipc'
import { listen } from '@tauri-apps/api/event'
import type {
  ConnectStage,
  FileTransferProgress,
  HostKeyVerifyRequest,
  ServerConnection,
  TerminalClosed,
  TerminalData,
  TerminalLogError,
  TunnelConfig,
  TunnelRuntime,
} from './contracts'
import { commands, sshEvents } from './contracts'
import type { InvokePayloads, Payloads, Results } from './contracts'

/** 统一调用 SSH 插件命令并保留每条命令的入参与返回类型。 */
function cmd<K extends keyof InvokePayloads & keyof Results>(
  command: K,
  payload: InvokePayloads[K]
): Promise<Results[K]> {
  return invokeCommand(command, payload)
}

export const ipc = {
  /* 连接（Rust 侧命令以 payload 对象为入参） */
  sshConnect: (p: Payloads['ssh_connect']) => cmd(commands.sshConnect, { payload: p }),
  sshDisconnect: (sessionId: string) => cmd(commands.sshDisconnect, { sessionId }),
  sshReconnect: (sessionId: string, overrides?: Payloads['ssh_reconnect']['overrides']) =>
    cmd(commands.sshReconnect, { sessionId, overrides }),
  sshConnections: () => cmd(commands.sshConnections, {}),

  /* 主机密钥（首连确认 / 指纹变更阻断） */
  sshHostKeyRespond: (p: Payloads['ssh_host_key_respond']) => cmd(commands.sshHostKeyRespond, p),
  sshKnownHostList: () => cmd(commands.sshKnownHostList, {}),
  sshKnownHostDelete: (p: Payloads['ssh_known_host_delete']) => cmd(commands.sshKnownHostDelete, p),

  /* 服务器配置 + 分组（配置与凭证引用持久化在后端插件库） */
  sshProfileList: () => cmd(commands.sshProfileList, {}),
  sshProfileSave: (p: Payloads['ssh_profile_save']) => cmd(commands.sshProfileSave, { payload: p }),
  sshProfileDelete: (profileId: string) => cmd(commands.sshProfileDelete, { profileId }),
  sshGroupList: () => cmd(commands.sshGroupList, {}),
  sshGroupSave: (group: Payloads['ssh_group_save']['group']) =>
    cmd(commands.sshGroupSave, { group }),
  sshGroupDelete: (groupId: string) => cmd(commands.sshGroupDelete, { groupId }),

  /* 隧道 */
  sshTunnelList: (profileId: string) => cmd(commands.sshTunnelList, { profileId }),
  sshTunnelSave: (config: TunnelConfig) => cmd(commands.sshTunnelSave, { config }),
  sshTunnelStart: (connectionId: string, tunnelId: string) =>
    cmd(commands.sshTunnelStart, { connectionId, tunnelId }),
  sshTunnelStop: (tunnelId: string) => cmd(commands.sshTunnelStop, { tunnelId }),
  sshTunnels: (connectionId: string) => cmd(commands.sshTunnels, { connectionId }),
  sshTunnelDelete: (tunnelId: string) => cmd(commands.sshTunnelDelete, { tunnelId }),

  /* 终端 */
  sshTerminalOpen: (p: Payloads['ssh_terminal_open']) => cmd(commands.sshTerminalOpen, p),
  sshTerminalWrite: (terminalId: string, data: string) =>
    cmd(commands.sshTerminalWrite, { terminalId, data }),
  terminalLogStart: (terminalId: string, dir: string | null) =>
    cmd(commands.sshTerminalLogStart, { terminalId, dir }),
  terminalLogStop: (terminalId: string) => cmd(commands.sshTerminalLogStop, { terminalId }),
  terminalLogOpenDir: () => cmd(commands.sshTerminalLogOpenDir, {}),
  sshTerminalResize: (terminalId: string, cols: number, rows: number) =>
    cmd(commands.sshTerminalResize, { terminalId, cols, rows }),
  sshTerminalClose: (terminalId: string) => cmd(commands.sshTerminalClose, { terminalId }),
  sshTerminalList: (connectionId: string) => cmd(commands.sshTerminalList, { connectionId }),

  /* 文件管理 */
  sshFileList: (connectionId: string, path: string) =>
    cmd(commands.sshFileList, { connectionId, path }),
  sshFileUpload: (p: Payloads['ssh_file_upload']) => cmd(commands.sshFileUpload, p),
  sshFileDownload: (p: Payloads['ssh_file_download']) => cmd(commands.sshFileDownload, p),
  sshFileDelete: (connectionId: string, remotePath: string, recursive?: boolean) =>
    cmd(commands.sshFileDelete, { connectionId, remotePath, recursive }),
  sshFileRename: (connectionId: string, oldPath: string, newPath: string) =>
    cmd(commands.sshFileRename, { connectionId, oldPath, newPath }),
  sshFileMkdir: (connectionId: string, path: string) =>
    cmd(commands.sshFileMkdir, { connectionId, path }),
  sshLocalList: (path: string) => cmd(commands.sshLocalList, { path }),
  sshLocalCreate: (path: string, isDir: boolean) => cmd(commands.sshLocalCreate, { path, isDir }),
  sshLocalDelete: (path: string, isDir: boolean) => cmd(commands.sshLocalDelete, { path, isDir }),
  sshLocalRename: (oldPath: string, newPath: string) =>
    cmd(commands.sshLocalRename, { oldPath, newPath }),
  sshFileCreate: (connectionId: string, remotePath: string) =>
    cmd(commands.sshFileCreate, { connectionId, remotePath }),
  sshFileChmod: (p: Payloads['ssh_file_chmod']) => cmd(commands.sshFileChmod, p),
  sshBookmarkList: (profileId: string) => cmd(commands.sshBookmarkList, { profileId }),
  sshBookmarkAdd: (profileId: string, name: string, path: string) =>
    cmd(commands.sshBookmarkAdd, { profileId, name, path }),
  sshBookmarkDelete: (id: string) => cmd(commands.sshBookmarkDelete, { id }),
  sshFileDownloadRecursive: (p: Payloads['ssh_file_download_recursive']) =>
    cmd(commands.sshFileDownloadRecursive, p),
  sshTransferCancel: (transferId: string) => cmd(commands.sshTransferCancel, { transferId }),

  /* 远程编辑 */
  sshEditOpen: (connectionId: string, remotePath: string) =>
    cmd(commands.sshEditOpen, { connectionId, remotePath }),
  sshEditSave: (
    connectionId: string,
    remotePath: string,
    content: string,
    expectedMtime?: number
  ) => cmd(commands.sshEditSave, { connectionId, remotePath, content, expectedMtime }),

  /* 监控 */
  sshMonitorGet: (connectionId: string) => cmd(commands.sshMonitorGet, { connectionId }),
  sshSystemInfoGet: (connectionId: string) => cmd(commands.sshSystemInfoGet, { connectionId }),

  /* 服务 */
  sshServiceList: (p: Payloads['ssh_service_list']) => cmd(commands.sshServiceList, p),
  sshServiceAction: (p: Payloads['ssh_service_action']) => cmd(commands.sshServiceAction, p),
  sshServiceLogs: (p: Payloads['ssh_service_logs']) => cmd(commands.sshServiceLogs, p),
  sshServiceConfig: (p: Payloads['ssh_service_config']) => cmd(commands.sshServiceConfig, p),

  /* 进程 */
  sshProcessList: (p: Payloads['ssh_process_list']) => cmd(commands.sshProcessList, p),
  sshProcessDetail: (connectionId: string, pid: number) =>
    cmd(commands.sshProcessDetail, { connectionId, pid }),
  sshProcessKill: (connectionId: string, pid: number, force?: boolean) =>
    cmd(commands.sshProcessKill, { connectionId, pid, force }),

  /* Docker */
  sshDockerList: (connectionId: string) => cmd(commands.sshDockerList, { connectionId }),
  sshDockerAction: (p: Payloads['ssh_docker_action']) => cmd(commands.sshDockerAction, p),
  sshDockerLogs: (p: Payloads['ssh_docker_logs']) => cmd(commands.sshDockerLogs, p),
  sshDockerExec: (p: Payloads['ssh_docker_exec']) => cmd(commands.sshDockerExec, { payload: p }),
}

/** 订阅终端数据（返回取消订阅函数） */
export function onTerminalData(fn: (d: TerminalData) => void): Promise<() => void> {
  return listen<TerminalData>(sshEvents.terminalData, (e) => fn(e.payload))
}

/** 订阅终端会话日志写盘失败（返回取消订阅函数） */
export function onTerminalLogError(fn: (d: TerminalLogError) => void): Promise<() => void> {
  return listen<TerminalLogError>(sshEvents.terminalLogError, (e) => fn(e.payload))
}

/** 订阅终端 PTY 通道关闭。 */
export function onTerminalClosed(fn: (d: TerminalClosed) => void): Promise<() => void> {
  return listen<TerminalClosed>(sshEvents.terminalClosed, (e) => fn(e.payload))
}

/** 订阅文件传输进度（返回取消订阅函数） */
export function onTransferProgress(fn: (d: FileTransferProgress) => void): Promise<() => void> {
  return listen<FileTransferProgress>(sshEvents.transferProgress, (e) => fn(e.payload))
}

/** 订阅连接状态（返回取消订阅函数） */
export function onConnectionStatus(fn: (d: ServerConnection) => void): Promise<() => void> {
  return listen<ServerConnection>(sshEvents.connectionStatus, (e) => fn(e.payload))
}

/** 订阅主机密钥确认请求（首连/指纹变更） */
export function onHostKeyVerify(fn: (d: HostKeyVerifyRequest) => void): Promise<() => void> {
  return listen<HostKeyVerifyRequest>(sshEvents.hostKeyVerify, (e) => fn(e.payload))
}

/** 订阅分阶段连接进度 */
export function onConnectStage(fn: (d: ConnectStage) => void): Promise<() => void> {
  return listen<ConnectStage>(sshEvents.connectStage, (e) => fn(e.payload))
}

/** 订阅隧道状态变化 */
export function onTunnelStatus(fn: (d: TunnelRuntime) => void): Promise<() => void> {
  return listen<TunnelRuntime>(sshEvents.tunnelStatus, (e) => fn(e.payload))
}
