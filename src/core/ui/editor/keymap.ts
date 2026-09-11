/**
 * 编辑器快捷键
 *
 * 与全局快捷键协调：`Ctrl+W`（关页签）、`Ctrl(Shift)+Tab`（切页签）在 `ToolWorkspace.vue`
 * 处理，编辑器不抢占。编辑器自己必须 `preventDefault` 并消费的键：
 * - `Mod-F` / `Mod-H`：否则 WebView2 打开自身查找栏；
 * - `Mod-S`：否则触发页面保存（WebView2 的"保存网页"）；
 * - `Mod-Z` / `Mod-Y` / `Mod-Shift-Z`：保证撤销走编辑器历史而不是页面级行为。
 */
import type { KeyBinding } from '@codemirror/view'
import {
  copyLineDown,
  copyLineUp,
  cursorMatchingBracket,
  deleteLine,
  indentLess,
  indentMore,
  moveLineDown,
  moveLineUp,
  redo,
  selectLine,
  toggleComment,
  undo,
} from '@codemirror/commands'
import { findNext, findPrevious, selectNextOccurrence } from '@codemirror/search'

/** 需要宿主（组件）处理的按键回调 */
export interface EditorKeymapHandlers {
  /** 打开查找面板（replace=true 时展开替换行） */
  openSearch: (replace: boolean) => void
  /** 关闭查找面板；返回 true 表示确实关闭（Esc 才算被消费） */
  closeSearch: () => boolean
  /** `Mod-S` 保存 */
  save: () => void
  /** `Mod-G` 跳转行（打开面板并聚焦输入框） */
  openGoToLine: () => void
}

/** 组装编辑器键位：宿主回调在前，其后是编辑命令 */
export function buildEditorKeymap(handlers: EditorKeymapHandlers): readonly KeyBinding[] {
  return [
    {
      key: 'Mod-f',
      run: () => {
        handlers.openSearch(false)
        return true
      },
    },
    {
      key: 'Mod-h',
      run: () => {
        handlers.openSearch(true)
        return true
      },
    },
    {
      key: 'Mod-s',
      run: () => {
        handlers.save()
        return true
      },
    },
    {
      key: 'Mod-g',
      run: () => {
        handlers.openGoToLine()
        return true
      },
    },
    { key: 'Escape', run: () => handlers.closeSearch() },
    { key: 'Mod-d', run: selectNextOccurrence, preventDefault: true },
    { key: 'Mod-/', run: toggleComment },
    { key: 'Alt-ArrowUp', run: moveLineUp },
    { key: 'Alt-ArrowDown', run: moveLineDown },
    { key: 'Shift-Alt-ArrowUp', run: copyLineUp },
    { key: 'Shift-Alt-ArrowDown', run: copyLineDown },
    { key: 'Mod-Shift-k', run: deleteLine },
    { key: 'Mod-l', run: selectLine },
    { key: 'Mod-]', run: indentMore },
    { key: 'Mod-[', run: indentLess },
    { key: 'Mod-Enter', run: cursorMatchingBracket },
    { key: 'Mod-z', run: undo, preventDefault: true },
    { key: 'Mod-y', run: redo, preventDefault: true },
    { key: 'Mod-Shift-z', run: redo, preventDefault: true },
    { key: 'F3', run: findNext, shift: findPrevious },
  ]
}

/** 键位说明（右键菜单与帮助展示用，保持与实际键位同源） */
export const EDITOR_KEY_HINTS: readonly { keys: string; label: string }[] = [
  { keys: 'Ctrl+F', label: '查找' },
  { keys: 'Ctrl+H', label: '替换' },
  { keys: 'Ctrl+G', label: '跳转行' },
  { keys: 'Ctrl+S', label: '保存' },
  { keys: 'Ctrl+/', label: '注释切换' },
  { keys: 'Alt+↑/↓', label: '移动行' },
  { keys: 'Shift+Alt+↑/↓', label: '复制行' },
  { keys: 'Ctrl+D', label: '选中下一个相同词' },
]
