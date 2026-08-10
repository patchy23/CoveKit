/**
 * 工具共享 · 复制到剪贴板（成功/失败 toast 反馈）
 */
import { useUiStore } from '@/stores/ui'

export function useCopy() {
  const ui = useUiStore()

  async function copyText(text: string, label = '已复制'): Promise<void> {
    if (!text) {
      ui.toast('没有可复制的内容')
      return
    }
    try {
      await navigator.clipboard.writeText(text)
      ui.toast(label)
    } catch {
      ui.toast('复制失败：请手动选择复制')
    }
  }

  return { copyText }
}
