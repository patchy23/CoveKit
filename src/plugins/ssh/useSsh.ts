/**
 * SSH 工具 · 状态管理与纯函数
 * 连接状态机 + 服务器配置本地持久化 + 展示格式化纯函数。
 */
import type {
  ServerProfile,
  ServerConnection,
  ConnectionStatus,
  SshGroup,
  TerminalSession,
  RemoteFile,
} from './contracts'

/* ── 连接状态机 ── */

export function statusDotClass(status: ConnectionStatus): string {
  switch (status) {
    case 'connected':
      return 'bg-success-strong dark:bg-success-dark'
    case 'connecting':
    case 'reconnecting':
      return 'bg-tertiary animate-pulse dark:bg-tertiary-dark'
    case 'error':
      return 'bg-danger-strong dark:bg-danger-dark'
    default:
      return 'bg-text-muted dark:bg-text-muted-dark'
  }
}

export function statusText(status: ConnectionStatus): string {
  switch (status) {
    case 'connected':
      return '已连接'
    case 'connecting':
      return '连接中'
    case 'reconnecting':
      return '重连中'
    case 'error':
      return '已断开'
    default:
      return '未连接'
  }
}

/* ── 假数据：服务器配置 ── */

export const mockProfiles: ServerProfile[] = [
  {
    id: 'profile-1',
    name: '生产服务器',
    host: '192.168.1.100',
    port: 22,
    username: 'root',
    authMethod: 'privateKey',
    remark: '主站 + API',
    lastConnectedAt: Date.now() - 3600000,
  },
  {
    id: 'profile-2',
    name: '测试环境',
    host: '10.0.0.5',
    port: 22,
    username: 'deploy',
    authMethod: 'password',
    lastConnectedAt: Date.now() - 86400000,
  },
  {
    id: 'profile-3',
    name: '开发机',
    host: '172.16.0.3',
    port: 2222,
    username: 'dev',
    authMethod: 'privateKeyWithPassphrase',
    remark: '本地 Kubernetes',
  },
]

/* ── localStorage 存量快照（一次性迁移到后端插件库；迁移成功后由调用方清理） ── */

const PROFILES_KEY = 'ssh.profiles.v1'
const GROUPS_KEY = 'ssh.groups.v1'

export interface LegacySnapshot {
  profiles: ServerProfile[]
  groups: SshGroup[]
}

/** 读取 localStorage 存量快照（secretRef → credentialRef 字段映射；无存量返回空） */
export function readLegacySnapshot(): LegacySnapshot {
  let profiles: ServerProfile[] = []
  let groups: SshGroup[] = []
  try {
    const rawProfiles = localStorage.getItem(PROFILES_KEY)
    if (rawProfiles) {
      profiles = (JSON.parse(rawProfiles) as (ServerProfile & { secretRef?: string })[]).map(
        ({ secretRef, ...rest }) => ({
          ...rest,
          credentialRef: secretRef,
        })
      )
    }
  } catch {
    /* 数据损坏时按空处理 */
  }
  try {
    const rawGroups = localStorage.getItem(GROUPS_KEY)
    if (rawGroups) {
      // 旧分组字段 order → sortOrder
      groups = (
        JSON.parse(rawGroups) as (SshGroup & { order?: number })[]
      ).map(({ order, ...rest }) => ({ ...rest, sortOrder: order ?? 0 }))
    }
  } catch {
    /* 数据损坏时按空处理 */
  }
  return { profiles, groups }
}

/** 清理 localStorage 存量快照（迁移成功后调用） */
export function clearLegacySnapshot(): void {
  localStorage.removeItem(PROFILES_KEY)
  localStorage.removeItem(GROUPS_KEY)
}

export const mockConnections: ServerConnection[] = [
  {
    profileId: 'profile-1',
    sessionId: 'conn-1',
    status: 'connected',
    host: '192.168.1.100',
    latencyMs: 23,
    connectedAt: Date.now() - 1800000,
  },
  {
    profileId: 'profile-2',
    sessionId: 'conn-2',
    status: 'disconnected',
  },
  {
    profileId: 'profile-3',
    sessionId: 'conn-3',
    status: 'connecting',
  },
]

/* ── 假数据：终端 ── */

export const mockTerminals: TerminalSession[] = [
  {
    id: 'term-1',
    connectionId: 'conn-1',
    title: '生产服务器',
    cols: 80,
    rows: 24,
    active: true,
  },
]

/* ── 假数据：文件列表 ── */

export const mockFiles: RemoteFile[] = [
  {
    name: '..',
    path: '/var/log',
    isDir: true,
    size: 0,
    modifiedAt: 0,
    permissions: 'drwxr-xr-x',
    owner: 'root',
    group: 'root',
  },
  {
    name: 'archive',
    path: '/var/log/nginx/archive',
    isDir: true,
    size: 0,
    modifiedAt: Date.now() - 604800000,
    permissions: 'drwxr-xr-x',
    owner: 'root',
    group: 'root',
  },
  {
    name: 'access.log',
    path: '/var/log/nginx/access.log',
    isDir: false,
    size: 13002342,
    modifiedAt: Date.now() - 3600000,
    permissions: '-rw-r--r--',
    owner: 'root',
    group: 'root',
  },
  {
    name: 'error.log',
    path: '/var/log/nginx/error.log',
    isDir: false,
    size: 3355443,
    modifiedAt: Date.now() - 7200000,
    permissions: '-rw-r--r--',
    owner: 'root',
    group: 'root',
  },
]

/* ── 工具函数 ── */

export { formatBytes } from '@/core/format'

export function formatTime(ts: number): string {
  if (!ts) return '-'
  const d = new Date(ts)
  const month = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  const hour = String(d.getHours()).padStart(2, '0')
  const min = String(d.getMinutes()).padStart(2, '0')
  return `${month}-${day} ${hour}:${min}`
}

export function formatLatency(ms?: number): string {
  if (ms === undefined) return '-'
  return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`
}

/** Docker 默认短 ID 为前 12 位；异常短值保持原样。 */
export function shortContainerId(id: string): string {
  return id.slice(0, 12)
}

/** 文件名和后缀不参与判断；不超过 10 MiB 的普通文件均可尝试按 UTF-8 打开。 */
export function canEditRemoteFile(file: RemoteFile): boolean {
  return !file.isDir && file.size <= 10 * 1024 * 1024
}
