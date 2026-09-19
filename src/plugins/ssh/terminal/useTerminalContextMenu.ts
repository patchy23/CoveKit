/**
 * SSH 终端右键菜单（从 TerminalTab 拆出，300 行红线）
 * 全选 / 复制 / 复制当前行 / 粘贴；菜单点击会带走 xterm 焦点，所有动作完成后统一归还焦点
 * （否则光标消失、键盘输入无响应）。
 */
import { computed, ref } from 'vue'
import type { Terminal } from 'xterm'
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager'
import { useUiStore } from '@/stores/ui'
import type { UiContextMenuItem } from '@/core/ui'

interface MenuState {
  x: number
  y: number
  hasSelection: boolean
  /** 右击位置所在逻辑行的完整文本（折行已合并；空串表示取不到内容，菜单项据此禁用） */
  line: string
}

export function useTerminalContextMenu(getTerm: () => Terminal | null) {
  const ui = useUiStore()
  const menu = ref<MenuState | null>(null)

  /**
   * 取某个视口行所在逻辑行的完整文本（折行合并）
   *
   * xterm 按终端宽度把一个逻辑行拆成多行，续行的 `isWrapped` 为 true：
   * 先向上回溯到逻辑行首，再向下收集到下一个非续行为止，拼起来才是整行内容。
   * 不合并的话，「复制当前行」在命令输出较长时只能拿到被折断的半截。
   */
  function lineTextAt(term: Terminal, viewportRow: number): string {
    const buffer = term.buffer.active
    const hitIndex = Math.max(0, buffer.viewportY + viewportRow)
    if (!buffer.getLine(hitIndex)) return ''

    let head = hitIndex
    while (head > 0 && buffer.getLine(head)?.isWrapped) head -= 1

    const parts: string[] = []
    for (let index = head; index < buffer.length; index += 1) {
      const line = buffer.getLine(index)
      if (!line) break
      // 逻辑行结束：出现不属于本行续行的行
      if (index > head && !line.isWrapped) break
      parts.push(line.translateToString(true))
    }
    return parts.join('').trimEnd()
  }

  /** 由点击坐标换算视口行号（终端渲染区按 rows 等分高度） */
  function viewportRowOf(event: MouseEvent, term: Terminal): number {
    const host = event.currentTarget as HTMLElement | null
    const rect = host?.getBoundingClientRect()
    if (!rect || rect.height <= 0 || term.rows <= 0) return 0
    const cellHeight = rect.height / term.rows
    const row = Math.floor((event.clientY - rect.top) / cellHeight)
    return Math.max(0, Math.min(row, term.rows - 1))
  }

  /** 在终端区域打开右键菜单（定位交给公共菜单；打开时即算好所在行文本） */
  function openContextMenu(event: MouseEvent) {
    event.preventDefault()
    const term = getTerm()
    menu.value = {
      x: event.clientX,
      y: event.clientY,
      hasSelection: Boolean(term?.hasSelection()),
      line: term ? lineTextAt(term, viewportRowOf(event, term)) : '',
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

  /** 复制右击那一行的完整文本（不含选区的场景：快速取某行命令或输出） */
  async function copyCurrentLine() {
    const term = getTerm()
    const text = menu.value?.line ?? ''
    if (!text) return
    try {
      await writeText(text)
      ui.toast('已复制当前行')
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

  const menuItems = computed<UiContextMenuItem[]>(() => [
    { label: '全选', onClick: selectAll },
    {
      label: '复制',
      disabled: !menu.value?.hasSelection,
      onClick: () => void copySelection(),
    },
    {
      label: '复制当前行',
      disabled: !menu.value?.line,
      onClick: () => void copyCurrentLine(),
    },
    { label: '粘贴', onClick: () => void pasteClipboard() },
  ])

  return { menu, menuItems, openContextMenu }
}
