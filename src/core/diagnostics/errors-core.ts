/**
 * 错误清单内核（可靠性 T11-4/T11-6）
 *
 * 与 `errors.ts`（全局钩子）分开的原因：内核只做有界缓冲与订阅，不碰 window/Vue，
 * 因此可以直接被单元测试覆盖；钩子那层只在真实应用里装一次。
 */

/** 错误清单条数上限（有界，长时间运行不会无限增长） */
export const MAX_ERROR_ENTRIES = 50

/** 单条面向用户说明的长度上限（防止把整段响应贴进来） */
export const MAX_MESSAGE_LEN = 300

/** 单条明细（堆栈）的长度上限 */
export const MAX_DETAIL_LEN = 600

/** 一条错误记录 */
export interface AppErrorEntry {
  /** 稳定错误码（如 ui.render_failed、update.check_failed） */
  code: string
  /** 面向用户的说明（已截断） */
  message: string
  /** 来源（模块或钩子名，如 update、vue:render） */
  source: string
  /** 明细（堆栈等，已截断；没有时为空串） */
  detail: string
  /** 同一条错误出现的次数（code + message 相同即合并） */
  count: number
  /** 首次出现时间（epoch 毫秒） */
  firstAt: number
  /** 最近出现时间（epoch 毫秒） */
  lastAt: number
}

/** 记录入参 */
export interface ErrorInput {
  /** 稳定错误码 */
  code: string
  /** 面向用户的说明 */
  message: string
  /** 来源 */
  source: string
  /** 明细（可选） */
  detail?: string
}

/** 有界清单：最新在前 */
const entries: AppErrorEntry[] = []
const listeners = new Set<(entries: AppErrorEntry[]) => void>()

/** 截断文本（超长加省略号，便于一眼看出被截断） */
function truncate(text: string, limit: number): string {
  if (text.length <= limit) return text
  return `${text.slice(0, limit)}…`
}

/** 通知订阅者（传只读副本，避免外部改内部状态） */
function notify(): void {
  const snapshot = entries.map((entry) => ({ ...entry }))
  for (const listener of listeners) {
    try {
      listener(snapshot)
    } catch (reason) {
      // 订阅者自身出错不能反过来打断记录流程
      console.error('[diagnostics] 错误清单订阅者异常', reason)
    }
  }
}

/**
 * 记录一条错误（code + message 相同的合并计数，并存到最前）。
 *
 * @returns 记录后的清单副本（最新在前）
 */
export function recordError(input: ErrorInput): AppErrorEntry[] {
  const code = input.code || 'app.unknown'
  const message = truncate(input.message || '未知错误', MAX_MESSAGE_LEN)
  const now = Date.now()
  const existing = entries.find((entry) => entry.code === code && entry.message === message)
  if (existing) {
    existing.count += 1
    existing.lastAt = now
    if (input.detail) existing.detail = truncate(input.detail, MAX_DETAIL_LEN)
  } else {
    entries.unshift({
      code,
      message,
      source: input.source,
      detail: truncate(input.detail ?? '', MAX_DETAIL_LEN),
      count: 1,
      firstAt: now,
      lastAt: now,
    })
    while (entries.length > MAX_ERROR_ENTRIES) {
      entries.pop()
    }
  }
  notify()
  return listErrors()
}

/** 当前清单副本（最新在前） */
export function listErrors(): AppErrorEntry[] {
  return entries.map((entry) => ({ ...entry }))
}

/** 清空清单（用户主动清空；返回是否清掉了内容） */
export function clearErrors(): boolean {
  if (entries.length === 0) return false
  entries.length = 0
  notify()
  return true
}

/** 订阅清单变化（返回退订函数） */
export function subscribeErrors(listener: (entries: AppErrorEntry[]) => void): () => void {
  listeners.add(listener)
  listener(listErrors())
  return () => {
    listeners.delete(listener)
  }
}

/** 把任意异常归一化成一个错误码（非 Error 对象用兜底码） */
export function normalizeUnknownError(
  reason: unknown,
  fallbackCode: string
): { code: string; message: string; detail: string } {
  if (reason instanceof Error) {
    return { code: fallbackCode, message: reason.message, detail: reason.stack ?? '' }
  }
  if (typeof reason === 'object' && reason !== null) {
    const record = reason as { code?: unknown; message?: unknown }
    const code = typeof record.code === 'string' && record.code ? record.code : fallbackCode
    const message = typeof record.message === 'string' ? record.message : String(reason)
    return { code, message, detail: '' }
  }
  return { code: fallbackCode, message: String(reason ?? '未知错误'), detail: '' }
}
