/**
 * 编辑器实例管理：EditorView 生命周期 + Compartment 热重配置 + 命令式 API
 *
 * 设计要点：
 * - 明暗、语言、只读、tabSize、换行、重能力、补全/校验一律走 Compartment 重配置，
 *   **绝不重建 EditorView**（重建会丢撤销历史与滚动位置）；
 * - 外部写入用 applyingExternal 标志抑制 change 回调；写入前先比对文档内容，避免自我循环；
 * - 语言包异步加载完成后若组件已卸载或请求已过期，丢弃结果（languageRequest 序号）；
 * - 大文件降级（>512KB 关重能力与语法高亮、>5MB 强制只读）只重配 Compartment，不重建实例；
 * - 查找条件由 `searchController` 持有时统一执行，避免「面板统计」与「实际跳转」两套逻辑。
 */
import { onBeforeUnmount, shallowRef, watch } from 'vue'
import { Compartment, EditorState, Prec, Transaction, type Extension } from '@codemirror/state'
import { EditorView, keymap } from '@codemirror/view'
import { indentUnit } from '@codemirror/language'
import { redo as redoCommand, undo as undoCommand } from '@codemirror/commands'
import { search as searchExtension } from '@codemirror/search'
import {
  buildBaseExtensions,
  completionEnabled,
  heavyExtensionsFor,
  highlightEnabled,
  linterEnabled,
} from './extensions'
import { buildEditorKeymap } from './keymap'
import { buildCompletion } from './completion'
import { linterForLanguage } from './lint'
import { formatDocument } from './format'
import { createDocStatsTracker } from './docStats'
import { createSearchController } from './searchController'
import { detectLanguage, loadLanguage, PLAIN_TEXT, type LanguageInfo } from './languages'
import type { EditorDegradeLevel } from './status'
import type { CodeEditorHandle, EditorCursorRange, UseCodeEditorOptions } from './types'

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
  /** 重能力：折叠 / 括号匹配 / 选区匹配 / 当前行高亮（大文件整块卸载） */
  const heavyCompartment = new Compartment()
  /** 补全与校验（随语言与降级级别重配） */
  const auxCompartment = new Compartment()
  /** 插件专用扩展（领域能力，如数据库的语句运行 gutter） */
  const extraCompartment = new Compartment()

  /** 外部写入标志：为 true 时忽略 docChanged 回调 */
  let applyingExternal = false
  /** 组件是否已销毁（异步语言加载后校验） */
  let destroyed = false
  /** 语言加载请求序号：只接受最后一次请求的结果 */
  let languageRequest = 0
  /** 「已保存」基线内容（未保存标记用） */
  let savedSnapshot = ''

  const docStats = createDocStatsTracker((level) => applyDegrade(level))
  const search = createSearchController(
    () => view.value,
    () => view.value?.focus()
  )

  /** 解析当前应使用的语言：显式 id 优先，'auto' 按文件名识别 */
  function resolveLanguage(): LanguageInfo {
    // 显式 language 优先，'auto' / 空串回退到文件名识别（标签统一取自 LANGUAGE_LABELS）
    return detectLanguage(options.filename(), options.language())
  }

  /** 应用语言扩展：上层注入优先（如 SQL 方言），否则按识别结果异步加载 */
  async function applyLanguage(): Promise<void> {
    const info = resolveLanguage()
    languageInfo.value = info
    const request = ++languageRequest
    const injected = options.languageExtension?.()
    // 空数组等价于「未注入」（调用方默认传 []），避免把空语言扩展当成有效配置
    const hasInjected = Array.isArray(injected) ? injected.length > 0 : Boolean(injected)
    if (hasInjected) {
      const current = view.value
      if (destroyed || !current) return
      current.dispatch({ effects: languageCompartment.reconfigure(injected ?? []) })
      return
    }
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

  /** 只读判定：props 只读，或文档超过 5MB 被强制只读 */
  function readOnlyNow(): boolean {
    return options.readonly() || docStats.level.value === 'huge'
  }

  /** 补全 + 校验扩展（随语言与降级级别变化重配） */
  function auxExtensions(): Extension {
    const extras: Extension[] = []
    if ((options.completion?.() ?? false) && completionEnabled(docStats.level.value)) {
      extras.push(buildCompletion({ sources: options.completionSources?.() ?? [] }))
    }
    if ((options.linter?.() ?? false) && linterEnabled(docStats.level.value)) {
      extras.push(...linterForLanguage(languageInfo.value.id))
    }
    return extras
  }

  /** 应用降级级别：只重配 Compartment，不重建实例 */
  function applyDegrade(level: EditorDegradeLevel): void {
    const current = view.value
    if (!current) return
    current.dispatch({
      effects: [
        heavyCompartment.reconfigure(
          heavyExtensionsFor(level, options.mode(), options.foldGutter())
        ),
        auxCompartment.reconfigure(auxExtensions()),
        editableCompartment.reconfigure(editableExtension(readOnlyNow())),
      ],
    })
    if (highlightEnabled(level)) void applyLanguage()
    else current.dispatch({ effects: languageCompartment.reconfigure([]) })
    if (level === 'huge') options.onError?.('文件超过 5MB，已强制只读')
  }

  /** 右键事件：把行号 / 行文本 / 选中文本交给宿主（不阻止默认行为，由宿主决定） */
  function handleContextMenu(event: MouseEvent, current: EditorView): boolean {
    if (!options.onContextMenu) return false
    const pos =
      current.posAtCoords({ x: event.clientX, y: event.clientY }) ??
      current.state.selection.main.head
    const line = current.state.doc.lineAt(pos)
    const range = current.state.selection.main
    options.onContextMenu({
      event,
      line: line.number,
      lineText: line.text,
      selection: current.state.sliceDoc(range.from, range.to),
    })
    return false
  }

  /** 同一个编辑器内创建新文档的独立 state，复用公共扩展契约。 */
  function createState(document: string): EditorState {
    return EditorState.create({
      doc: document,
      extensions: [
        Prec.high(
          keymap.of(
            buildEditorKeymap({
              openSearch: (replace) => options.onRequestSearch?.(replace),
              closeSearch: () => options.onRequestClosePanel?.() ?? false,
              save: () => options.onSave?.(),
              openGoToLine: () => options.onRequestGoToLine?.(),
            })
          )
        ),
        ...buildBaseExtensions({
          mode: options.mode(),
          lineNumbers: options.lineNumbers(),
          placeholder: options.placeholder(),
        }),
        searchExtension(),
        heavyCompartment.of(heavyExtensionsFor('none', options.mode(), options.foldGutter())),
        languageCompartment.of([]),
        auxCompartment.of([]),
        extraCompartment.of(options.extraExtensions?.() ?? []),
        editableCompartment.of(editableExtension(options.readonly())),
        tabSizeCompartment.of(indentExtension(options.tabSize())),
        wrappingCompartment.of(wrappingExtension(options.lineWrapping())),
        EditorView.domEventHandlers({ contextmenu: handleContextMenu }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged && !applyingExternal) {
            options.onChange(update.state.doc.toString())
          }
          if (update.docChanged) docStats.update(update.state)
          // 文档或选区变化都会影响「当前是第几个匹配」，条件为空时 refresh 内部直接返回
          if (update.docChanged || update.selectionSet) search.refresh()
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
  }

  /** 创建 EditorView（仅执行一次） */
  function mount(): void {
    const parent = host.value
    if (!parent || view.value) return
    destroyed = false
    languageInfo.value = resolveLanguage()

    const state = createState(options.modelValue())

    view.value = new EditorView({ parent, state })
    savedSnapshot = state.doc.toString()
    docStats.init(state)
    view.value.dispatch({ effects: auxCompartment.reconfigure(auxExtensions()) })
    void applyLanguage()
  }

  /** 销毁 EditorView */
  function destroy(): void {
    destroyed = true
    documents.clear()
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

  /** 读取选区与光标偏移（插件封装组件按选区取文本用） */
  function getCursor(): EditorCursorRange {
    const current = view.value
    if (!current) return { from: 0, to: 0, head: 0 }
    const range = current.state.selection.main
    return { from: range.from, to: range.to, head: range.head }
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

  /** 按当前语言格式化：成功后写入并进撤销历史（可一次 Ctrl+Z 撤回） */
  function format(): { ok: boolean; error?: string } {
    const current = view.value
    if (!current) return { ok: false, error: '编辑器尚未就绪' }
    const result = formatDocument(
      current.state.doc.toString(),
      languageInfo.value.id,
      options.tabSize()
    )
    if (!result.ok) return { ok: false, error: result.error }
    writeValue(result.output, true)
    return { ok: true }
  }

  /** 记录「已保存」基线 */
  function markSaved(): void {
    savedSnapshot = view.value?.state.doc.toString() ?? options.modelValue()
  }

  /** 相对基线是否有未保存修改 */
  function isDirty(): boolean {
    return getValue() !== savedSnapshot
  }

  // 文档身份与文本一起观察，先保存旧 state 再同步新文本，避免跨页污染撤销栈。
  const documents = new Map<
    string,
    { state: EditorState; top: number; left: number; saved: string }
  >()
  let activeDocument = options.documentKey?.() ?? ''
  watch([() => options.documentKey?.() ?? '', options.modelValue], ([key, value]) => {
    const current = view.value
    if (!current) return
    if (key !== activeDocument) {
      if (
        activeDocument &&
        (!options.documentKeys?.() || options.documentKeys().includes(activeDocument))
      )
        documents.set(activeDocument, {
          state: current.state,
          top: current.scrollDOM.scrollTop,
          left: current.scrollDOM.scrollLeft,
          saved: savedSnapshot,
        })
      const cached = documents.get(key)
      current.setState(cached?.state ?? createState(value))
      activeDocument = key
      savedSnapshot = cached?.saved ?? value
      current.scrollDOM.scrollTop = cached?.top ?? 0
      current.scrollDOM.scrollLeft = cached?.left ?? 0
      docStats.init(current.state)
      current.dispatch({
        effects: [
          extraCompartment.reconfigure(options.extraExtensions?.() ?? []),
          auxCompartment.reconfigure(auxExtensions()),
          editableCompartment.reconfigure(editableExtension(readOnlyNow())),
          tabSizeCompartment.reconfigure(indentExtension(options.tabSize())),
          wrappingCompartment.reconfigure(wrappingExtension(options.lineWrapping())),
        ],
      })
      void applyLanguage()
      search.refresh()
    }
    writeValue(value, false)
  })
  watch(
    () => options.documentKeys?.(),
    (keys) => {
      if (!keys) return
      const allowed = new Set(keys)
      for (const key of documents.keys()) if (!allowed.has(key)) documents.delete(key)
    }
  )

  // 语言 / 文件名变化 → 重新识别并懒加载（校验与补全随之重配）
  watch([options.language, options.filename, () => options.languageExtension?.()], () => {
    if (!view.value) return
    void applyLanguage()
    view.value.dispatch({ effects: auxCompartment.reconfigure(auxExtensions()) })
  })

  // 只读切换（huge 级强制只读优先）
  watch(options.readonly, () => {
    const current = view.value
    if (!current) return
    current.dispatch({
      effects: editableCompartment.reconfigure(editableExtension(readOnlyNow())),
    })
  })

  // 补全 / 校验开关
  watch(
    [() => options.completion?.(), () => options.linter?.(), () => options.completionSources?.()],
    () => {
      view.value?.dispatch({ effects: auxCompartment.reconfigure(auxExtensions()) })
    }
  )

  // 插件专用扩展变化（上层注入的领域能力，如数据库的语句运行 gutter）
  watch(
    () => options.extraExtensions?.(),
    (extensions) => {
      view.value?.dispatch({ effects: extraCompartment.reconfigure(extensions ?? []) })
    }
  )

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
    docStats: docStats.stats,
    degrade: docStats.level,
    searchState: search.state,
    mount,
    destroy,
    getValue,
    setValue,
    focus,
    blur,
    goToLine,
    getSelection,
    getCursor,
    setCursor: (from: number, to: number) => {
      const current = view.value
      if (!current) return
      const clamp = (offset: number) =>
        Math.max(0, Math.min(Math.trunc(offset) || 0, current.state.doc.length))
      current.dispatch({
        selection: { anchor: clamp(from), head: clamp(to) },
        scrollIntoView: true,
      })
    },
    insert,
    undo,
    redo,
    applySearch: search.apply,
    findNext: search.next,
    findPrevious: search.previous,
    replaceCurrent: search.replaceCurrent,
    replaceAllMatches: search.replaceAllMatches,
    clearSearch: search.clear,
    format,
    markSaved,
    isDirty,
  }
}
