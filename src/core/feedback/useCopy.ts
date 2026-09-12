/**
 * 应用反馈 · 带提示的复制
 *
 * 组合 `core/platform/clipboard`（平台调用）与 ui store（通知），
 * 工具与页面只依赖本入口即可获得「复制成功/空值/失败」三种可见反馈。
 */
import { useUiStore } from '@/stores/ui'
import { writeClipboardText } from '@/core/platform/clipboard'

/**
 * 复制文本到剪贴板并给出可见反馈。
 *
 * @returns copyText(text, label?)：空值提示「没有可复制的内容」，成功提示 label，失败提示手抄兜底
 */
export function useCopy() {
  const ui = useUiStore()

  async function copyText(text: string, label = '已复制'): Promise<void> {
    const result = await writeClipboardText(text)
    if (result.ok) {
      ui.toast(label)
      return
    }
    ui.toast(result.reason === 'empty' ? '没有可复制的内容' : '复制失败：请手动选择复制')
  }

  return { copyText }
}
