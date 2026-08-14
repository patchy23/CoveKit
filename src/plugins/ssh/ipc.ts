/**
 * SSH 工具插件 · IPC 封装（本插件命令与事件，独立于框架）
 * 命令命名与 Rust 侧 serde（camelCase）一致；事件经 listen 订阅。
 */
import { invokeCommand } from '@/core/ipc/ipc'
import { listen } from '@tauri-apps/api/event'
import type {
  FileTransferProgress,
  ServerConnection,
  TerminalClosed,
  TerminalData,
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
  sshReconnect: (sessionId: string, creds: Omit<Payloads['ssh_reconnect'], 'sessionId'>) =>
    cmd(commands.sshReconnect, { sessionId, payload: creds }),
  sshConnections: () => cmd(commands.sshConnections, {}),

  /* 终端 */
  sshTerminalOpen: (p: Payloads['ssh_terminal_open']) => cmd(commands.sshTerminalOpen, p),
  sshTerminalWrite: (terminalId: string, data: string) =>
    cmd(commands.sshTerminalWrite, { terminalId, data }),
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

  /* 凭证 */
  sshCredentialSave: (p: Payloads['ssh_credential_save']) =>
    cmd(commands.sshCredentialSave, { payload: p }),
  sshCredentialGet: (profileId: string) => cmd(commands.sshCredentialGet, { profileId }),
  sshCredentialDelete: (profileId: string) => cmd(commands.sshCredentialDelete, { profileId }),

  /* 远程编辑 */
  sshEditOpen: (connectionId: string, remotePath: string) =>
    cmd(commands.sshEditOpen, { connectionId, remotePath }),
  sshEditSave: (connectionId: string, remotePath: string, content: string) =>
    cmd(commands.sshEditSave, { connectionId, remotePath, content }),

  /* 监控 */
  sshMonitorGet: (connectionId: string) => cmd(commands.sshMonitorGet, { connectionId }),

  /* 服务 */
  sshServiceList: (p: Payloads['ssh_service_list']) => cmd(commands.sshServiceList, p),
  sshServiceAction: (p: Payloads['ssh_service_action']) => cmd(commands.sshServiceAction, p),
  sshServiceLogs: (p: Payloads['ssh_service_logs']) => cmd(commands.sshServiceLogs, p),

  /* 进程 */
  sshProcessList: (p: Payloads['ssh_process_list']) => cmd(commands.sshProcessList, p),
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
