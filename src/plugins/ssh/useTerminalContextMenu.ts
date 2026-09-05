/**
 * SSH 终端右键菜单（从 TerminalTab 拆出，300 行红线）
 * 全选/复制/粘贴；菜单点击会带走 xterm 焦点，所有动作完成后统一归还焦点（否则光标消失、键盘输入无响应）。
 */
import { computed, ref } from 'vue'
import type { Terminal } from 'xterm'
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager'
import { useUiStore } from '@/stores/ui'
import type { ContextMenuItem } from '@/core/ui/ContextMenu.vue'

interface MenuState {
  x: number
  y: number
  hasSelection: boolean
}

export function useTerminalContextMenu(getTerm: () => Terminal | null) {
  const ui = useUiStore()
  const menu = ref<MenuState | null>(null)

  /** 在终端区域打开右键菜单（视口边界 clamp） */
  function openContextMenu(event: MouseEvent) {
    event.preventDefault()
    const width = 150
    const height = 112
    menu.value = {
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - width - 8)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - height - 8)),
      hasSelection: Boolean(getTerm()?.hasSelection()),
    }
  }

  function selectAll() {
    getTerm()?.selectAll()
    getTerm()?.focus()
  }

  async function copySelection() {
    const term = getTerm()
    const selection = term?.getSelection() ?? ''
    if (!selection) return
    try {
      await writeText(selection)
    } catch (error) {
      ui.toast(`复制失败：${error}`)
    } finally {
      term?.focus()
    }
  }

  async function pasteClipboard() {
    const term = getTerm()
    try {
      const text = await readText()
      if (text) term?.paste(text)
    } catch (error) {
      ui.toast(`粘贴失败：${error}`)
    } finally {
      term?.focus()
    }
  }

  const menuItems = computed<ContextMenuItem[]>(() => [
    { label: '全选', onClick: selectAll },
    {
      label: '复制',
      disabled: !menu.value?.hasSelection,
      onClick: () => void copySelection(),
    },
    { label: '粘贴', onClick: () => void pasteClipboard() },
  ])

  return { menu, menuItems, openContextMenu }
}
