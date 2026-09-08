/** SSH 契约 · 监控与服务/进程/Docker */

/* ── SSH 契约 · 监控与服务/进程/Docker ── */

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
