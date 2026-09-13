/**
 * 本地诊断报告（可靠性 T11-5）
 *
 * 口径：**只生成本地文本，用户主动复制才离开本机**，不做任何网络上传。
 *
 * 白名单字段（其余一律不带）：
 * - 应用版本与标识、运行平台、Tauri/系统信息；
 * - 存储与凭证的保护模式（系统密钥库 / 降级文件），**不带任何路径与密钥材料**；
 * - 活跃任务数与任务类型、最近结束任务的结果；
 * - 错误码汇总（code × 次数，最近几条的说明）。
 *
 * 路径脱敏：涉及路径的字段一律不采集；报告里出现的路径段会被替换成 `<path>`，
 * 以免用户把诊断粘给别人时泄漏本机目录结构。
 */
import { getVersion, getTauriVersion } from '@tauri-apps/api/app'
import { ipc } from '@/core/ipc/ipc'
import { listErrors } from './errors-core'

/** 报告字段（便于测试与 UI 复用） */
export interface DiagnosticsReport {
  /** 应用版本 */
  appVersion: string
  /** Tauri 运行时版本 */
  tauriVersion: string
  /** 运行平台（如 windows） */
  platform: string
  /** 存储根是否为默认目录（不看路径本身） */
  storageIsDefault: boolean
  /** 存储分区数量（四分区布局为 4） */
  storagePartitions: number
  /** 是否编译进系统密钥库原生后端 */
  nativeProtection: boolean
  /** 各保护域的模式描述（不含路径与密钥） */
  protectionDomains: string[]
  /** 活跃任务数 */
  activeTasks: number
  /** 最近结束的任务摘要 */
  recentTasks: string[]
  /** 错误码汇总（code × 次数） */
  errorCodes: string[]
  /** 最近错误的说明（最新在前） */
  recentErrors: string[]
}

/** 路径脱敏：把常见的绝对路径片段替换掉（Windows 盘符、Unix 家目录、UNC） */
export function redactPaths(text: string): string {
  return text
    .replace(/[A-Za-z]:\\\\[^\s"'，。；:]+/g, '<path>')
    .replace(/[A-Za-z]:\\[^\s"'，。；:]+/g, '<path>')
    .replace(/\/(?:Users|home|var|tmp|opt|mnt)\/[^\s"'，。；:]+/g, '<path>')
    .replace(/\\\\[^\s"'，。；:]+/g, '<path>')
}

/** 采集诊断字段（任一项失败都退化成可读的占位文本，不让整个报告失败） */
export async function collectDiagnostics(): Promise<DiagnosticsReport> {
  const errors = listErrors()
  const report: DiagnosticsReport = {
    appVersion: '未知',
    tauriVersion: '未知',
    platform: '未知',
    storageIsDefault: false,
    storagePartitions: 0,
    nativeProtection: false,
    protectionDomains: [],
    activeTasks: 0,
    recentTasks: [],
    errorCodes: [],
    recentErrors: [],
  }

  try {
    report.appVersion = await getVersion()
    report.tauriVersion = await getTauriVersion()
  } catch (reason) {
    report.appVersion = `未知（${reason instanceof Error ? reason.message : String(reason)}）`
  }

  // 平台用 webview 自报的标识（Windows 为 Win32、macOS 为 MacIntel…）：
  // 诊断只需要能区分平台，为此引入额外系统插件不值得
  const uaData = (navigator as { userAgentData?: { platform?: string } }).userAgentData
  report.platform = uaData?.platform ?? navigator.platform ?? '未知'

  try {
    const storage = await ipc.storageInfo()
    report.storageIsDefault = storage.isDefault
    report.storagePartitions = storage.partitions.length
  } catch (reason) {
    report.storageIsDefault = false
    report.storagePartitions = 0
    report.recentErrors.push(
      `storage_info 失败：${reason instanceof Error ? reason.message : String(reason)}`
    )
  }

  try {
    const protection = await ipc.vaultProtectionStatus()
    report.nativeProtection = protection.nativeBackend
    report.protectionDomains = protection.domains.map(
      (domain) => `${domain.domain}：${domain.backend}（${domain.availability}）`
    )
  } catch (reason) {
    report.protectionDomains = [
      `保护状态未知：${reason instanceof Error ? reason.message : String(reason)}`,
    ]
  }

  try {
    const tasks = await ipc.frameworkTasks()
    report.activeTasks = tasks.active.length
    report.recentTasks = [...tasks.active, ...tasks.finished].slice(0, 3).map((task) => {
      const progress = task.progress === null ? '' : ` ${task.progress}%`
      const failure = task.error ? ` ${task.error.code}` : ''
      return `${task.kind}(${task.id}) ${task.state}${progress}${failure}`
    })
  } catch (reason) {
    report.recentTasks = [
      `任务清单未知：${reason instanceof Error ? reason.message : String(reason)}`,
    ]
  }

  report.errorCodes = errors.map((entry) => `${entry.code} × ${entry.count}`)
  report.recentErrors = errors.slice(0, 5).map((entry) => `[${entry.code}] ${entry.message}`)
  return report
}

/** 把报告渲染成可直接粘贴的文本（白名单字段；路径已脱敏） */
export function formatDiagnostics(report: DiagnosticsReport): string {
  const lines = [
    'CoveKit 诊断信息（本地生成，未上传）',
    `应用版本：${report.appVersion}`,
    `Tauri 版本：${report.tauriVersion}`,
    `平台：${report.platform}`,
    `存储根：${report.storageIsDefault ? '默认目录' : '自定义目录'}（${report.storagePartitions} 个分区）`,
    `系统密钥库保护：${report.nativeProtection ? '已启用' : '未启用（当前为降级文件保护）'}`,
  ]
  if (report.protectionDomains.length > 0) {
    lines.push(`保护域：${report.protectionDomains.join('；')}`)
  }
  lines.push(`活跃任务数：${report.activeTasks}`)
  if (report.recentTasks.length > 0) {
    lines.push(`最近任务：${report.recentTasks.join('；')}`)
  }
  if (report.errorCodes.length > 0) {
    lines.push(`错误码：${report.errorCodes.join('；')}`)
  }
  if (report.recentErrors.length > 0) {
    lines.push('最近错误：')
    lines.push(...report.recentErrors.map((entry) => `- ${entry}`))
  }
  return redactPaths(lines.join('\n'))
}
