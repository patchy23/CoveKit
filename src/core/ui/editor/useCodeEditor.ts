/**
 * 编辑器实例管理：EditorView 生命周期 + Compartment 热重配置 + 命令式 API
 *
 * 设计要点：
 * - 明暗、语言、只读、tabSize、换行一律走 Compartment 重配置，**绝不重建 EditorView**
 *   （重建会丢撤销历史与滚动位置）；
 * - 外部写入用 applyingExternal 标志抑制 change 回调，避免上层把程序性变更误判成用户编辑；
 *   写入前先比对文档内容，避免自我循环；
 * - 语言包异步加载完成后若组件已卸载或请求已过期，丢弃结果（languageRequest 序号）。
 */
import { onBeforeUnmount, shallowRef, watch } from 'vue'
import { Compartment, EditorState, Transaction, type Extension } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { indentUnit } from '@codemirror/language'
import { redo as redoCommand, undo as undoCommand } from '@codemirror/commands'
import { buildBaseExtensions } from './extensions'
import { detectLanguage, loadLanguage, PLAIN_TEXT, type LanguageInfo } from './languages'
import type { CodeEditorHandle, UseCodeEditorOptions } from './types'

/** 创建编辑器实例管理（须在 setup 作用域内调用） */
export function useCodeEditor(options: UseCodeEditorOptions): CodeEditorHandle {
  const host = shallowRef<HTMLElement | null>(null)
  const view = shallowRef<EditorView | null>(null)
  const languageInfo = shallowRef<LanguageInfo>(PLAIN_TEXT)

  /** 语言扩展（异步加载后热替换） */
  const languageCompartment = new Compartment()
  /** 只读 / 可编辑 */
  const editableCompartment = new Compartment()
  /** 缩进宽度（tabSize 与 indentUnit 必须同步，否则 Tab 与自动缩进宽度不一致） */
  const tabSizeCompartment = new Compartment()
  /** 长行换行 */
  const wrappingCompartment = new Compartment()

  /** 外部写入标志：为 true 时忽略 docChanged 回调 */
  let applyingExternal = false
  /** 组件是否已销毁（异步语言加载后校验） */
  let destroyed = false
  /** 语言加载请求序号：只接受最后一次请求的结果 */
  let languageRequest = 0

  /** 解析当前应使用的语言：显式 id 优先，'auto' 按文件名识别 */
  function resolveLanguage(): LanguageInfo {
    const explicit = options.language()
    if (explicit && explicit !== 'auto') return { id: explicit, label: explicit }
    return detectLanguage(options.filename())
  }

  /** 异步加载语言扩展并热重配置（失败退化为纯文本） */
  async function applyLanguage(): Promise<void> {
    const info = resolveLanguage()
    languageInfo.value = info
    const request = ++languageRequest
    const extension = await loadLanguage(info, options.filename())
    const current = view.value
    if (destroyed || !current || request !== languageRequest) return
    current.dispatch({ effects: languageCompartment.reconfigure(extension) })
  }

  /** 只读 / 可编辑组合扩展 */
  function editableExtension(readonly: boolean): Extension {
    return readonly
      ? [EditorView.editable.of(false), EditorState.readOnly.of(true)]
      : EditorView.editable.of(true)
  }

  /** 换行扩展 */
  function wrappingExtension(wrapping: boolean): Extension {
    return wrapping ? EditorView.lineWrapping : []
  }

  /** 缩进配置（tabSize + 等宽缩进单位） */
  function indentExtension(size: number): Extension {
    return [EditorState.tabSize.of(size), indentUnit.of(' '.repeat(size))]
  }

  /** 创建 EditorView（仅执行一次） */
  function mount(): void {
    const parent = host.value
    if (!parent || view.value) return
    destroyed = false
    languageInfo.value = resolveLanguage()

    const state = EditorState.create({
      doc: options.modelValue(),
      extensions: [
        ...buildBaseExtensions({
          mode: options.mode(),
          lineNumbers: options.lineNumbers(),
          foldGutter: options.foldGutter(),
          placeholder: options.placeholder(),
        }),
        languageCompartment.of([]),
        editableCompartment.of(editableExtension(options.readonly())),
        tabSizeCompartment.of(indentExtension(options.tabSize())),
        wrappingCompartment.of(wrappingExtension(options.lineWrapping())),
        EditorView.updateListener.of((update) => {
          if (update.docChanged && !applyingExternal) {
            options.onChange(update.state.doc.toString())
          }
          if (!options.onCursor || !(update.selectionSet || update.docChanged)) return
          const range = update.state.selection.main
          const line = update.state.doc.lineAt(range.head)
          options.onCursor({
            line: line.number,
            column: range.head - line.from + 1,
            selected: range.to - range.from,
          })
        }),
      ],
    })

    view.value = new EditorView({ parent, state })
    void applyLanguage()
  }

  /** 销毁 EditorView */
  function destroy(): void {
    destroyed = true
    view.value?.destroy()
    view.value = null
  }

  /** 全量写入文档（统一入口）：addHistory=true 时进撤销历史（用户级替换） */
  function writeValue(value: string, addHistory: boolean): void {
    const current = view.value
    if (!current || current.state.doc.toString() === value) return
    applyingExternal = true
    try {
      current.dispatch({
        changes: { from: 0, to: current.state.doc.length, insert: value },
        annotations: addHistory ? undefined : Transaction.addToHistory.of(false),
      })
    } finally {
      applyingExternal = false
    }
  }

  /** 写入内容（默认不进撤销历史） */
  function setValue(value: string, writeOptions?: { addToHistory?: boolean }): void {
    writeValue(value, writeOptions?.addToHistory ?? false)
  }

  /** 读取内容（未挂载时回落到 props 值） */
  function getValue(): string {
    return view.value?.state.doc.toString() ?? options.modelValue()
  }

  /** 聚焦编辑器 */
  function focus(): void {
    view.value?.focus()
  }

  /** 让编辑器失焦 */
  function blur(): void {
    view.value?.contentDOM.blur()
  }

  /** 跳转到指定行（1 起始，越界自动收敛） */
  function goToLine(line: number): void {
    const current = view.value
    if (!current) return
    const target = Math.min(Math.max(Math.trunc(line), 1), current.state.doc.lines)
    const pos = current.state.doc.line(target).from
    current.dispatch({
      selection: { anchor: pos },
      effects: EditorView.scrollIntoView(pos, { y: 'center' }),
    })
    current.focus()
  }

  /** 读取选中文本 */
  function getSelection(): string {
    const current = view.value
    if (!current) return ''
    const range = current.state.selection.main
    return current.state.sliceDoc(range.from, range.to)
  }

  /** 在光标处插入文本（用户级操作，进撤销历史） */
  function insert(text: string): void {
    const current = view.value
    if (!current) return
    const range = current.state.selection.main
    current.dispatch({
      changes: { from: range.from, to: range.to, insert: text },
      selection: { anchor: range.from + text.length },
    })
    current.focus()
  }

  /** 撤销 */
  function undo(): void {
    if (!view.value) return
    view.value.focus()
    undoCommand(view.value)
  }

  /** 重做 */
  function redo(): void {
    if (!view.value) return
    view.value.focus()
    redoCommand(view.value)
  }

  // 外部值变化 → 同步进编辑器（writeValue 内部抑制 change 回调）
  watch(options.modelValue, (value) => writeValue(value, false))

  // 语言 / 文件名变化 → 重新识别并懒加载
  watch([options.language, options.filename], () => {
    if (view.value) void applyLanguage()
  })

  // 只读切换
  watch(options.readonly, (readonly) => {
    view.value?.dispatch({ effects: editableCompartment.reconfigure(editableExtension(readonly)) })
  })

  // 缩进宽度切换
  watch(options.tabSize, (size) => {
    view.value?.dispatch({ effects: tabSizeCompartment.reconfigure(indentExtension(size)) })
  })

  // 换行切换
  watch(options.lineWrapping, (wrapping) => {
    view.value?.dispatch({ effects: wrappingCompartment.reconfigure(wrappingExtension(wrapping)) })
  })

  onBeforeUnmount(destroy)

  return {
    host,
    view,
    languageInfo,
    mount,
    destroy,
    getValue,
    setValue,
    focus,
    blur,
    goToLine,
    getSelection,
    insert,
    undo,
    redo,
  }
}
