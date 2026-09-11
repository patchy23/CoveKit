/**
 * frp 状态与日志的展示映射（纯函数：文案 key、语义色、日志等级、环形缓冲）
 * 集中在此便于单测锁定，组件只做渲染不做判定。
 */
import type { FrpLogStream, FrpStateName } from '../contracts'

/** 状态展示视图（i18n key + 语义色档 + 是否中间态） */
export interface FrpStatusView {
  /** i18n 文案 key（frp.stateStopped / stateStarting / stateRunning / stateError） */
  labelKey: string
  /** 语义色档（对齐 UiBadge / UiTone） */
  tone: 'neutral' | 'success' | 'warning' | 'danger'
  /** 状态点样式（12px 圆点） */
  dotClass: string
  /** 是否处于中间态（启动/停止过程中，按钮需禁用） */
  transient: boolean
}

/** 状态 → 展示视图（四态全覆盖，未知态按 stopped 兜底） */
export function statusView(state: FrpStateName): FrpStatusView {
  switch (state) {
    case 'starting':
      return {
        labelKey: 'frp.stateStarting',
        tone: 'warning',
        dotClass: 'bg-warning-strong animate-pulse',
        transient: true,
      }
    case 'running':
      return {
        labelKey: 'frp.stateRunning',
        tone: 'success',
        dotClass: 'bg-success-strong',
        transient: false,
      }
    case 'error':
      return {
        labelKey: 'frp.stateError',
        tone: 'danger',
        dotClass: 'bg-danger-strong',
        transient: false,
      }
    default:
      return {
        labelKey: 'frp.stateStopped',
        tone: 'neutral',
        dotClass: 'bg-text-muted',
        transient: false,
      }
  }
}

/** 日志等级（决定着色） */
export type FrpLogLevel = 'error' | 'warn' | 'info'

/** 前端持有的日志行（附等级，避免渲染时反复正则） */
export interface FrpLogLine {
  /** 时间戳（Unix 毫秒） */
  ts: number
  /** 日志正文（已剥 ANSI、已脱敏） */
  line: string
  /** 来源流 */
  stream: FrpLogStream
  /** 等级 */
  level: FrpLogLevel
}

/**
 * 从日志行识别等级。
 * frpc 自身有 `[I]/[W]/[E]` 标记；无标记的行按关键字兜底（`start error`、`login to server failed` 等）。
 */
export function logLevel(line: string): FrpLogLevel {
  if (
    /\[E\]/.test(line) ||
    /\b(error|failed|failure|refused|incorrect|denied|unauthorized|forbidden|already in use|bind)\b/i.test(
      line
    )
  ) {
    return 'error'
  }
  if (/\[W\]/.test(line) || /\bwarn(ing)?\b/i.test(line)) return 'warn'
  return 'info'
}

/** 日志行等级 → 文本色 class（浅色/暗色各一档，均满足对比度） */
export function logLevelClass(level: FrpLogLevel): string {
  switch (level) {
    case 'error':
      return 'text-danger-strong dark:text-danger-dark'
    case 'warn':
      return 'text-warning-strong dark:text-warning-dark'
    default:
      return 'text-secondary dark:text-secondary-dark'
  }
}

/**
 * 追加日志行并裁剪到上限（返回新数组，纯函数）。
 * `max <= 0` 视为不限制；超出时保留最新 `max` 行（环形缓冲语义）。
 */
export function appendLogLine(buffer: FrpLogLine[], line: FrpLogLine, max: number): FrpLogLine[] {
  const limit = max > 0 ? max : Number.MAX_SAFE_INTEGER
  if (buffer.length < limit) {
    return [...buffer, line]
  }
  // 超限：丢掉最旧的一行再追加（长度恒定，避免整块 slice）
  const next = buffer.slice(buffer.length - limit + 1)
  next.push(line)
  return next
}
