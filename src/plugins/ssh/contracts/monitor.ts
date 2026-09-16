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

/* ── 远程系统信息与磁盘明细 ── */

/** 远程系统信息（缺失字段为空/0，界面显示 `-`） */

export interface SshSystemInfo {
  /** 主机名（hostname） */

  hostname: string

  /** 发行版名（/etc/os-release 的 PRETTY_NAME） */

  osName: string

  /** 内核信息（uname -srm） */

  kernel: string

  /** 运行时长文本（uptime 的 `up` 段，如 `42 days, 20:05`） */

  uptimeText: string

  /** 1/5/15 分钟平均负载 */

  loadAvg: [number, number, number]

  /** 逻辑 CPU 核数（nproc） */

  cpuCores: number
}

/** 磁盘分区条目（df -hlPT 一行） */

export interface SshDiskEntry {
  /** 文件系统名（/dev/vda1、overlay、tmpfs） */

  filesystem: string

  /** 文件系统类型（ext4/xfs/tmpfs/overlay） */

  fsType: string

  /** 总量（人类可读原文，如 `99G`） */

  sizeText: string

  /** 已用（人类可读原文） */

  usedText: string

  /** 可用（人类可读原文） */

  availText: string

  /** 使用率 0-100（df 的 `-` 视为 0） */

  usePercent: number

  /** 挂载点 */

  mountPoint: string
}

/** 系统信息采集结果（ssh_system_info_get 出参） */

export interface SshSystemInfoResult {
  /** 系统信息 */

  info: SshSystemInfo

  /** 磁盘分区明细 */

  disks: SshDiskEntry[]
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

/** 单进程详情（后端 `ps -fp` 输出解析，远端 ps 实现不同时字段可能缺失） */

export interface ProcessDetail {
  /** 进程 ID（查询值回显） */

  pid: number

  /** 远端是否存在该进程 */

  found: boolean

  /** 运行用户（BSD 列序下无此列） */

  user?: string

  /** 父进程 ID */

  ppid?: number

  /** 控制终端（无终端为 `?`） */

  tty?: string

  /** 启动时间列（ps 原样） */

  started?: string

  /** 累计 CPU 时间（ps 原样） */

  cpuTime?: string

  /** 完整命令行 */

  command?: string

  /** ps 原始输出（含报错文本，展示兜底） */

  raw: string
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
