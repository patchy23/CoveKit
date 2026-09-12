/**
 * 平台能力 · 剪贴板
 *
 * 只负责平台调用并返回结果，不做用户提示（提示由 `core/feedback` 适配），
 * 使基础 UI 与业务层都能复用同一份复制实现而不引入通知依赖。
 */

/** 复制结果：`empty` = 无内容可复制（正常缺省，不是错误）；`failed` = 平台写入失败 */
export type ClipboardWriteResult = { ok: true } | { ok: false; reason: 'empty' | 'failed' }

/**
 * 写入文本到系统剪贴板。
 *
 * @param text 待写入文本；空串按 `empty` 返回，调用方据此给「没有可复制的内容」而非报错。
 */
export async function writeClipboardText(text: string): Promise<ClipboardWriteResult> {
  if (!text) return { ok: false, reason: 'empty' }
  try {
    await navigator.clipboard.writeText(text)
    return { ok: true }
  } catch {
    return { ok: false, reason: 'failed' }
  }
}
