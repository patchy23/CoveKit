/**
 * 编辑器契约类型
 *
 * 集中定义对外形状：光标信息、实例管理入参、命令式句柄。
 * 实现留在 `useCodeEditor.ts`，避免「契约 + 实现」混在一个文件里越写越长。
 */
import type { Ref } from 'vue'
import type { EditorView } from '@codemirror/view'
import type { LanguageInfo } from './languages'
import type { EditorMode } from './extensions'

/** 光标位置信息（状态栏与上层联动用） */
export interface EditorCursorInfo {
  /** 行号（1 起始） */
  line: number
  /** 列号（1 起始） */
  column: number
  /** 当前选中字符数（无选中为 0） */
  selected: number
}

/** useCodeEditor 需要的响应式取值函数与回调（组件把 props/emit 适配进来） */
export interface UseCodeEditorOptions {
  /** 当前值 */
  modelValue: () => string
  /** 文件名（language='auto' 时用于识别语言） */
  filename: () => string | undefined
  /** 语言 id 或 'auto' */
  language: () => string | undefined
  /** 只读 */
  readonly: () => boolean
  /** 档位 */
  mode: () => EditorMode
  /** 行号开关 */
  lineNumbers: () => boolean
  /** 折叠 gutter 开关（full 档） */
  foldGutter: () => boolean
  /** 长行换行开关 */
  lineWrapping: () => boolean
  /** 缩进宽度 */
  tabSize: () => number
  /** 占位提示 */
  placeholder: () => string | undefined
  /** 用户编辑回调（外部写入不触发） */
  onChange: (value: string) => void
  /** 光标变化回调 */
  onCursor?: (info: EditorCursorInfo) => void
}

/** 编辑器实例控制句柄 */
export interface CodeEditorHandle {
  /** 挂载容器（模板 ref 绑定） */
  host: Ref<HTMLElement | null>
  /** 当前 EditorView（未挂载时为 null） */
  view: Ref<EditorView | null>
  /** 当前语言信息（异步识别结果） */
  languageInfo: Ref<LanguageInfo>
  /** 创建编辑器（onMounted 调用） */
  mount: () => void
  /** 销毁编辑器 */
  destroy: () => void
  /** 读取内容 */
  getValue: () => string
  /** 写入内容（默认不进撤销历史） */
  setValue: (value: string, options?: { addToHistory?: boolean }) => void
  /** 聚焦 */
  focus: () => void
  /** 失焦 */
  blur: () => void
  /** 跳转到指定行（1 起始） */
  goToLine: (line: number) => void
  /** 读取选中文本 */
  getSelection: () => string
  /** 在光标处插入文本 */
  insert: (text: string) => void
  /** 撤销 */
  undo: () => void
  /** 重做 */
  redo: () => void
}
