<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
/**
 * 通用代码编辑器（编辑器组件的唯一对外入口）
 *
 * - `readonly` 时即查看器（取代原 CodeViewer），语义与视觉保持一致；
 * - 明暗、语言、只读、缩进、换行、重能力、补全/校验全部走 Compartment 热重配置，切换不重建实例；
 * - `mode="minimal"` 为轻量档：替代原「行号 + textarea」输入输出区（无折叠 / 括号闭合 / 列选择）；
 * - 查找替换（Ctrl+F / Ctrl+H）、跳转行（Ctrl+G）、保存（Ctrl+S）由编辑器内键位触发，
 *   浮层渲染在编辑器容器内，条件与统计由 `editor/searchController` 单一来源驱动；
 * - 大文件自动降级：>512KB 关语法高亮与折叠，>5MB 强制只读，并在状态栏与 `error` 事件中提示。
 */
import { computed, nextTick, ref, watch } from 'vue'
import type { CompletionSource } from '@codemirror/autocomplete'
import type { Extension } from '@codemirror/state'
import { useCodeEditor } from './editor/useCodeEditor'
import EditorGoToLineBar from './editor/EditorGoToLineBar.vue'
import EditorSearchBar from './editor/EditorSearchBar.vue'
import EditorStatusBar from './editor/EditorStatusBar.vue'
import { buildStatusText } from './editor/status'
import type { SearchOptions } from './editor/search'
import type { EditorMode } from './editor/extensions'
import type { EditorContextMenuPayload, EditorCursorInfo } from './editor/types'

const props = withDefaults(
  defineProps<{
    /** 多文档宿主的身份与存活集合，用于独立保留撤销栈和滚动位置。 */
    documentKey?: string
    documentKeys?: string[]
    /** 编辑内容（v-model） */
    modelValue?: string
    /** 语言 id；默认 'auto' 表示按 filename 识别 */
    language?: string
    /** 直接注入语言扩展（优先于识别；用于 SQL 方言这类参数化语言） */
    languageExtension?: Extension | Extension[]
    /** 文件名或路径：language='auto' 时用于识别，亦用于状态栏展示 */
    filename?: string
    /** 只读（查看器形态，快捷键与折叠仍可用） */
    readonly?: boolean
    /** 档位：full 完整能力，minimal 轻量输入区 */
    mode?: EditorMode
    /** 是否显示行号 */
    lineNumbers?: boolean
    /** 是否显示折叠 gutter（仅 full 档生效） */
    foldGutter?: boolean
    /** 长行是否换行 */
    lineWrapping?: boolean
    /** 缩进宽度（空格数） */
    tabSize?: number
    /** 空文档占位提示 */
    placeholder?: string
    /** 容器高度（CSS 长度值） */
    height?: string
    /** 是否启用查找替换（Ctrl+F / Ctrl+H 与浮层面板） */
    searchable?: boolean
    /** 是否显示底部状态栏（行列 / 选中 / 语言 / 缩进 / 规模 / 编码） */
    statusBar?: boolean
    /** 是否启用语法校验（错误波浪线 + 中文提示） */
    lint?: boolean
    /** 是否启用补全（注入源优先，其后是文档词法兜底） */
    completion?: boolean
    /** 注入的补全源（数据库表名 / 列名等） */
    completionSources?: CompletionSource[]
    /** 插件专用扩展（领域能力：语句运行 gutter、Ctrl+点击表名跳转等） */
    extraExtensions?: Extension[]
  }>(),
  {
    documentKey: undefined,
    documentKeys: undefined,
    modelValue: '',
    language: 'auto',
    languageExtension: undefined,
    filename: undefined,
    readonly: false,
    mode: 'full',
    lineNumbers: true,
    foldGutter: true,
    lineWrapping: false,
    tabSize: 2,
    placeholder: undefined,
    height: '100%',
    searchable: true,
    statusBar: false,
    lint: true,
    completion: true,
    completionSources: undefined,
    extraExtensions: undefined,
  }
)

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'change', value: string): void
  (event: 'cursor', info: EditorCursorInfo): void
  (event: 'save'): void
  (event: 'contextmenu', payload: EditorContextMenuPayload): void
  (event: 'error', message: string): void
}>()

/** 编辑器挂载容器 */
const hostRef = ref<HTMLDivElement | null>(null)
/** 光标信息（状态栏用） */
const cursor = ref<EditorCursorInfo>({ line: 1, column: 1, selected: 0 })
/** 查找浮层开关 */
const searchOpen = ref(false)
/** 打开查找时是否展开替换行 */
const replaceMode = ref(false)
/** 跳转行浮层开关 */
const goToLineOpen = ref(false)
const searchBar = ref<InstanceType<typeof EditorSearchBar> | null>(null)
const goToLineBar = ref<InstanceType<typeof EditorGoToLineBar> | null>(null)

const editor = useCodeEditor({
  documentKey: () => props.documentKey ?? '',
  documentKeys: () => props.documentKeys ?? [],
  modelValue: () => props.modelValue ?? '',
  filename: () => props.filename,
  language: () => props.language,
  languageExtension: () => props.languageExtension ?? [],
  readonly: () => props.readonly,
  mode: () => props.mode,
  lineNumbers: () => props.lineNumbers,
  foldGutter: () => props.foldGutter,
  lineWrapping: () => props.lineWrapping,
  tabSize: () => props.tabSize,
  placeholder: () => props.placeholder,
  completion: () => props.completion,
  completionSources: () => props.completionSources ?? [],
  extraExtensions: () => props.extraExtensions ?? [],
  linter: () => props.lint,
  onChange: (value) => {
    emit('update:modelValue', value)
    emit('change', value)
  },
  onCursor: (info) => {
    cursor.value = info
    emit('cursor', info)
  },
  onSave: () => emit('save'),
  onContextMenu: (payload) => emit('contextmenu', payload),
  onError: (message) => emit('error', message),
  onRequestSearch: (replace) => openSearch(replace),
  onRequestGoToLine: () => openGoToLine(),
  onRequestClosePanel: () => closePanel(),
})

/** 状态栏文案（纯函数拼装） */
const status = computed(() =>
  buildStatusText({
    cursor: cursor.value,
    lines: editor.docStats.value.lines,
    length: editor.docStats.value.length,
    languageLabel: editor.languageInfo.value.label,
    tabSize: props.tabSize,
    readonly: props.readonly || editor.degrade.value === 'huge',
    degrade: editor.degrade.value,
  })
)

/** 打开查找浮层（Ctrl+F / Ctrl+H） */
function openSearch(replace: boolean): void {
  if (!props.searchable) return
  replaceMode.value = replace
  goToLineOpen.value = false
  searchOpen.value = true
  void nextTick(() => searchBar.value?.focus())
}

/** 打开跳转行浮层（Ctrl+G） */
function openGoToLine(): void {
  searchOpen.value = false
  goToLineOpen.value = true
  void nextTick(() => goToLineBar.value?.focus())
}

/** 关闭浮层；返回是否确实关闭了（Esc 键位消费判定） */
function closePanel(): boolean {
  if (goToLineOpen.value) {
    goToLineOpen.value = false
    editor.focus()
    return true
  }
  if (searchOpen.value) {
    searchOpen.value = false
    editor.clearSearch()
    editor.focus()
    return true
  }
  return false
}

/** 查找条件变化 → 交给控制器执行 */
function onSearch(payload: { query: string; replacement: string; options: SearchOptions }): void {
  editor.applySearch(payload.query, payload.replacement, payload.options)
}

/** 跳转行确认 */
function onGoToLine(line: number): void {
  goToLineOpen.value = false
  editor.goToLine(line)
}

/** 格式化：失败以 `error` 事件暴露（调用方决定 toast 文案） */
function format(): boolean {
  const result = editor.format()
  if (!result.ok && result.error) emit('error', result.error)
  return result.ok
}

// 插槽或弹窗可能延后提供宿主；以真实 DOM 就绪为准，不依赖一次性的 mounted。
watch(
  hostRef,
  (host) => {
    editor.destroy()
    editor.host.value = host
    if (host) editor.mount()
  },
  { flush: 'post' }
)

defineExpose({
  /** 跨窗口迁移只保存可序列化视图位置，不导出编辑器内部实例。 */
  getViewport: () => {
    const view = editor.view.value
    return view
      ? {
          from: view.state.selection.main.anchor,
          to: view.state.selection.main.head,
          top: view.scrollDOM.scrollTop,
          left: view.scrollDOM.scrollLeft,
        }
      : undefined
  },
  restoreViewport: (position: { from: number; to: number; top: number; left: number }) => {
    editor.setCursor(position.from, position.to)
    const view = editor.view.value
    if (view)
      view.requestMeasure({
        write: () => {
          view.scrollDOM.scrollTop = position.top
          view.scrollDOM.scrollLeft = position.left
        },
      })
  },
  /** 聚焦 */
  focus: () => editor.focus(),
  /** 失焦 */
  blur: () => editor.blur(),
  /** 读取当前内容 */
  getValue: () => editor.getValue(),
  /** 写入内容（默认不进撤销历史） */
  setValue: (value: string, options?: { addToHistory?: boolean }) =>
    editor.setValue(value, options),
  /** 跳转到指定行（1 起始） */
  goToLine: (line: number) => editor.goToLine(line),
  /** 读取选中文本 */
  getSelection: () => editor.getSelection(),
  /** 读取选区与光标偏移（文档偏移） */
  getCursor: () => editor.getCursor(),
  setCursor: (from: number, to: number) => editor.setCursor(from, to),
  /** 在光标处插入文本 */
  insert: (text: string) => editor.insert(text),
  /** 撤销 */
  undo: () => editor.undo(),
  /** 重做 */
  redo: () => editor.redo(),
  /** 打开查找浮层（replace=true 时展开替换行） */
  find: (replace = false) => openSearch(replace),
  /** 按当前语言格式化；返回是否成功（失败同时触发 error 事件） */
  format,
  /** 记录当前内容为「已保存」基线 */
  markSaved: () => editor.markSaved(),
  /** 相对基线是否有未保存修改 */
  isDirty: () => editor.isDirty(),
  /** 当前语言中文标签（自动识别或显式指定） */
  getLanguageLabel: () => editor.languageInfo.value.label,
  /** 当前查找状态（匹配总数 / 序号 / 错误） */
  getSearchState: () => editor.searchState.value,
})
</script>

<template>
  <div
    class="ui-code-editor flex flex-col overflow-hidden rounded-lg border border-border bg-surface dark:border-border-dark dark:bg-surface-dark"
    :style="{ height }"
  >
    <div class="relative min-h-0 flex-1">
      <UiScrollArea axis="vertical" managed class="h-full">
        <div ref="hostRef" class="h-full w-full" />
      </UiScrollArea>

      <div v-if="searchOpen" class="absolute top-2 right-3 z-20">
        <EditorSearchBar
          ref="searchBar"
          :total="editor.searchState.value.total"
          :current="editor.searchState.value.current"
          :error="editor.searchState.value.error"
          :replace-mode="replaceMode"
          :readonly="readonly"
          @search="onSearch"
          @next="editor.findNext()"
          @previous="editor.findPrevious()"
          @replace="editor.replaceCurrent()"
          @replace-all="editor.replaceAllMatches()"
          @close="closePanel()"
        />
      </div>

      <div v-if="goToLineOpen" class="absolute top-2 right-3 z-20">
        <EditorGoToLineBar
          ref="goToLineBar"
          :current-line="cursor.line"
          :total-lines="editor.docStats.value.lines"
          @confirm="onGoToLine"
          @close="closePanel()"
        />
      </div>
    </div>

    <EditorStatusBar v-if="statusBar" :status="status" />
  </div>
</template>
