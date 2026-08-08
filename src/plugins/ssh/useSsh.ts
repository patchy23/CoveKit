/**
 * SSH 工具 · 状态管理与纯函数
 * 连接状态机 + 会话列表 + 服务器配置（当前为前端假数据，后端 IPC 接入后替换）
 */
import type {
  ServerProfile,
  ServerConnection,
  ConnectionStatus,
  TerminalSession,
  RemoteFile,
} from "./contracts";

/* ── 连接状态机 ── */

export function statusDotClass(status: ConnectionStatus): string {
  switch (status) {
    case "connected":
      return "bg-success-strong dark:bg-success-dark";
    case "connecting":
    case "reconnecting":
      return "bg-tertiary animate-pulse dark:bg-tertiary-dark";
    case "error":
      return "bg-danger-strong dark:bg-danger-dark";
    default:
      return "bg-text-muted dark:bg-text-muted-dark";
  }
}

export function statusText(status: ConnectionStatus): string {
  switch (status) {
    case "connected":
      return "已连接";
    case "connecting":
      return "连接中";
    case "reconnecting":
      return "重连中";
    case "error":
      return "已断开";
    default:
      return "未连接";
  }
}

/* ── 假数据：服务器配置 ── */

export const mockProfiles: ServerProfile[] = [
  {
    id: "profile-1",
    name: "生产服务器",
    host: "192.168.1.100",
    port: 22,
    username: "root",
    authMethod: "privateKey",
    remark: "主站 + API",
    lastConnectedAt: Date.now() - 3600000,
  },
  {
    id: "profile-2",
    name: "测试环境",
    host: "10.0.0.5",
    port: 22,
    username: "deploy",
    authMethod: "password",
    lastConnectedAt: Date.now() - 86400000,
  },
  {
    id: "profile-3",
    name: "开发机",
    host: "172.16.0.3",
    port: 2222,
    username: "dev",
    authMethod: "privateKeyWithPassphrase",
    remark: "本地 Kubernetes",
  },
];

export const mockConnections: ServerConnection[] = [
  {
    profileId: "profile-1",
    sessionId: "conn-1",
    status: "connected",
    host: "192.168.1.100",
    latencyMs: 23,
    connectedAt: Date.now() - 1800000,
  },
  {
    profileId: "profile-2",
    sessionId: "conn-2",
    status: "disconnected",
  },
  {
    profileId: "profile-3",
    sessionId: "conn-3",
    status: "connecting",
  },
];

/* ── 假数据：终端 ── */

export const mockTerminals: TerminalSession[] = [
  {
    id: "term-1",
    connectionId: "conn-1",
    title: "生产服务器",
    cols: 80,
    rows: 24,
    active: true,
  },
];

/* ── 假数据：文件列表 ── */

export const mockFiles: RemoteFile[] = [
  {
    name: "..",
    path: "/var/log",
    isDir: true,
    size: 0,
    modifiedAt: 0,
    permissions: "drwxr-xr-x",
    owner: "root",
    group: "root",
  },
  {
    name: "archive",
    path: "/var/log/nginx/archive",
    isDir: true,
    size: 0,
    modifiedAt: Date.now() - 604800000,
    permissions: "drwxr-xr-x",
    owner: "root",
    group: "root",
  },
  {
    name: "access.log",
    path: "/var/log/nginx/access.log",
    isDir: false,
    size: 13002342,
    modifiedAt: Date.now() - 3600000,
    permissions: "-rw-r--r--",
    owner: "root",
    group: "root",
  },
  {
    name: "error.log",
    path: "/var/log/nginx/error.log",
    isDir: false,
    size: 3355443,
    modifiedAt: Date.now() - 7200000,
    permissions: "-rw-r--r--",
    owner: "root",
    group: "root",
  },
];

/* ── 工具函数 ── */

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(2)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function formatTime(ts: number): string {
  if (!ts) return "-";
  const d = new Date(ts);
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  const hour = String(d.getHours()).padStart(2, "0");
  const min = String(d.getMinutes()).padStart(2, "0");
  return `${month}-${day} ${hour}:${min}`;
}

export function formatLatency(ms?: number): string {
  if (ms === undefined) return "-";
  return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
}
