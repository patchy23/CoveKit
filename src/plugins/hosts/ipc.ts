/**
 * hosts 修改插件 · IPC 封装（本插件命令，独立于框架）
 */
import { invokeCommand } from '@/core/ipc/ipc'
import type { HostsResult } from './contracts'

export const ipc = {
  hostsRead: (): Promise<HostsResult> => invokeCommand('hosts_read', {}),
  hostsSave: (content: string): Promise<HostsResult> => invokeCommand('hosts_save', { content }),
}
