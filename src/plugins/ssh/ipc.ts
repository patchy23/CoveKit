/**
 * SSH 工具插件 · IPC 封装（本插件命令与事件，独立于框架）
 * 命令命名与 Rust 侧 serde（camelCase）一致；事件经 listen 订阅。
 */
import { invokeCommand } from "@/core/ipc/ipc";
import { listen } from "@tauri-apps/api/event";
import type { Payloads, Results, ServerConnection, TerminalData } from "./contracts";
import { sshEvents } from "./contracts";

/** 调命令（返回类型按契约；入参序列化时校验） */
async function cmd<K extends keyof Payloads & keyof Results>(
  name: K,
  payload: Record<string, unknown>,
): Promise<Results[K]> {
  return invokeCommand(name, payload);
}

export const ipc = {
  /* 连接（Rust 侧命令以 payload 对象为入参） */
  sshConnect: (p: Payloads["ssh_connect"]) => cmd("ssh_connect", { payload: p }),
  sshDisconnect: (sessionId: string) => cmd("ssh_disconnect", { sessionId }),
  sshReconnect: (sessionId: string, creds: Omit<Payloads["ssh_reconnect"], "sessionId">) =>
    cmd("ssh_reconnect", { sessionId, payload: creds }),
  sshConnections: () => cmd("ssh_connections", {}),

  /* 终端 */
  sshTerminalOpen: (p: Payloads["ssh_terminal_open"]) => cmd("ssh_terminal_open", p),
  sshTerminalWrite: (terminalId: string, data: string) =>
    cmd("ssh_terminal_write", { terminalId, data }),
  sshTerminalResize: (terminalId: string, cols: number, rows: number) =>
    cmd("ssh_terminal_resize", { terminalId, cols, rows }),
  sshTerminalClose: (terminalId: string) => cmd("ssh_terminal_close", { terminalId }),
  sshTerminalList: (connectionId: string) => cmd("ssh_terminal_list", { connectionId }),

  /* 文件管理 */
  sshFileList: (connectionId: string, path: string) => cmd("ssh_file_list", { connectionId, path }),
  sshFileUpload: (p: Payloads["ssh_file_upload"]) => cmd("ssh_file_upload", p),
  sshFileDownload: (p: Payloads["ssh_file_download"]) => cmd("ssh_file_download", p),
  sshFileDelete: (connectionId: string, remotePath: string, recursive?: boolean) =>
    cmd("ssh_file_delete", { connectionId, remotePath, recursive }),
  sshFileRename: (connectionId: string, oldPath: string, newPath: string) =>
    cmd("ssh_file_rename", { connectionId, oldPath, newPath }),

  /* 凭证 */
  sshCredentialSave: (p: Payloads["ssh_credential_save"]) =>
    cmd("ssh_credential_save", { payload: p }),
  sshCredentialGet: (profileId: string) => cmd("ssh_credential_get", { profileId }),
  sshCredentialDelete: (profileId: string) => cmd("ssh_credential_delete", { profileId }),

  /* 远程编辑 */
  sshEditOpen: (connectionId: string, remotePath: string) =>
    cmd("ssh_edit_open", { connectionId, remotePath }),
  sshEditSave: (connectionId: string, remotePath: string, content: string) =>
    cmd("ssh_edit_save", { connectionId, remotePath, content }),

  /* 监控 */
  sshMonitorGet: (connectionId: string) => cmd("ssh_monitor_get", { connectionId }),

  /* 服务 */
  sshServiceList: (p: Payloads["ssh_service_list"]) => cmd("ssh_service_list", p),
  sshServiceAction: (p: Payloads["ssh_service_action"]) => cmd("ssh_service_action", p),
  sshServiceLogs: (p: Payloads["ssh_service_logs"]) => cmd("ssh_service_logs", p),

  /* 进程 */
  sshProcessList: (p: Payloads["ssh_process_list"]) => cmd("ssh_process_list", p),
  sshProcessKill: (connectionId: string, pid: number, force?: boolean) =>
    cmd("ssh_process_kill", { connectionId, pid, force }),

  /* Docker */
  sshDockerList: (connectionId: string) => cmd("ssh_docker_list", { connectionId }),
  sshDockerAction: (p: Payloads["ssh_docker_action"]) => cmd("ssh_docker_action", p),
  sshDockerLogs: (p: Payloads["ssh_docker_logs"]) => cmd("ssh_docker_logs", p),
  sshDockerExec: (p: Payloads["ssh_docker_exec"]) => cmd("ssh_docker_exec", p),
};

/** 订阅终端数据（返回取消订阅函数） */
export function onTerminalData(fn: (d: TerminalData) => void): Promise<() => void> {
  return listen<TerminalData>(sshEvents.terminalData, (e) => fn(e.payload));
}

/** 订阅文件传输进度（返回取消订阅函数） */
export function onTransferProgress(fn: (d: { transferId: string; fileName: string; transferred: number; total: number; done: boolean }) => void): Promise<() => void> {
  return listen(sshEvents.transferProgress, (e) => fn(e.payload as never));
}

/** 订阅连接状态（返回取消订阅函数） */
export function onConnectionStatus(fn: (d: ServerConnection) => void): Promise<() => void> {
  return listen(sshEvents.connectionStatus, (e) => fn(e.payload as never));
}
