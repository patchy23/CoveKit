<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
import UiButton from './UiButton.vue'
/**
 * 代码差异视图（L3 档，基于 `@codemirror/merge`）
 *
 * - `mode="split"`：左右两栏对照（只读），变更行高亮 + 中缝连接线；
 * - `mode="unified"`：单栏内联展示变更块，`readonly=false` 时块上出现接受/拒绝控件；
 * - 顶部按行内容计数给出新增/删除统计（`diff.ts`），视图负责具体变更块的对齐。
 *
 * 两份文本共用异步加载的语言扩展；正文按变化区间更新，配置尽量就地更新，保留阅读现场。
 * 形态或内联控件变化仍重建视图。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  getOriginalDoc,
  MergeView,
  originalDocChangeEffect,
  setExternalChunks,
  unifiedMergeView,
} from '@codemirror/merge'
import { ChangeSet, Compartment, EditorState, type Extension } from '@codemirror/state'
import { EditorView, lineNumbers } from '@codemirror/view'
import { syntaxHighlighting } from '@codemirror/language'
import { codeEditorTheme, codeHighlightStyle } from './editor/theme'
import { detectLanguage, loadLanguage } from './editor/languages'
import { createDiffTask, restoreChunks, type DiffResult } from './editor/asyncDiff'
import type { Text } from '@codemirror/state'
import { documentChange } from './editor/documentChange'

/** diff 展示形态 */
export type DiffMode = 'split' | 'unified'

const props = withDefaults(
  defineProps<{
    /** 原文本 */
    original?: string
    /** 新文本 */
    modified?: string
    /** 展示形态 */
    mode?: DiffMode
    /** 文件名（用于语言识别） */
    filename?: string
    /** 显式指定语言（优先于文件名识别） */
    language?: string
    /** 只读：split 恒只读；unified 只读时隐藏接受/拒绝控件 */
    readonly?: boolean
    /** 高度 */
    height?: string
    /** 是否显示顶部统计 */
    showStats?: boolean
  }>(),
  {
    original: '',
    modified: '',
    mode: 'split',
    filename: undefined,
    language: undefined,
    readonly: true,
    height: '320px',
    showStats: true,
  }
)

const host = ref<HTMLDivElement | null>(null)
/** 按行内容计数的差异统计，不代替视图的变更块对齐。 */
const stats = ref({ added: 0, removed: 0, same: true })
const calculating = ref(false)
const calculationError = ref('')
const diffTask = createDiffTask()
let diffEpoch = 0
let versionA = 0
let versionB = 0
let documentA: Text | undefined
let documentB: Text | undefined
let queued = false

/** 事务先安装正文并撤销旧块，再按不可变双文档快照计算；同一轮更新只计算一次。 */
function refreshDiff() {
  queued = false
  const target = mergeView ?? unifiedView
  if (!target) return
  const original = mergeView ? mergeView.a.state.doc : getOriginalDoc(unifiedView!.state)
  const modified = mergeView ? mergeView.b.state.doc : unifiedView!.state.doc
  if (original === documentA && modified === documentB) return
  if (original !== documentA) versionA++
  if (modified !== documentB) versionB++
  documentA = original
  documentB = modified
  const a = versionA
  const b = versionB
  const epoch = ++diffEpoch
  calculationError.value = ''
  function apply(result: DiffResult) {
    if (epoch !== diffEpoch) return
    if (result.versionA !== a || result.versionB !== b)
      throw new Error('差异计算返回了不匹配的文档版本')
    if (setExternalChunks(target!, original, modified, restoreChunks(result))) {
      stats.value = result.stats
      calculating.value = false
    }
  }
  function fail(error: unknown) {
    if (epoch !== diffEpoch) return
    calculating.value = false
    if (!(error instanceof Error && error.name === 'AbortError'))
      calculationError.value = error instanceof Error ? error.message : String(error)
  }
  try {
    const result = diffTask.run({
      original: original.toString(),
      modified: modified.toString(),
      versionA: a,
      versionB: b,
    })
    calculating.value = result instanceof Promise
    if (result instanceof Promise) void result.then(apply).catch(fail)
    else apply(result)
  } catch (error) {
    fail(error)
  }
}

function scheduleDiff() {
  if (queued) return
  queued = true
  queueMicrotask(() => {
    if (queued) refreshDiff()
  })
}

function cancelDiff() {
  diffEpoch++
  diffTask.cancel()
  calculating.value = false
  calculationError.value = '差异计算已取消，正文仍可阅读'
}

function retryDiff() {
  documentA = undefined
  documentB = undefined
  refreshDiff()
}
/** 形态文案 */
const modeLabel = computed(() => (props.mode === 'unified' ? '内联对比' : '左右对照'))

let mergeView: MergeView | null = null
let unifiedView: EditorView | null = null
/** 创建请求序号：快速切换 props 时只接受最后一次 create 的结果，防止异步交错叠出双视图 */
let createRequest = 0
const languageCompartment = new Compartment()
let languageExtensions: Extension = []
let rendered: ReturnType<typeof configuration> | null = null

function configuration() {
  return {
    original: props.original,
    modified: props.modified,
    mode: props.mode,
    filename: props.filename,
    language: props.language,
    readonly: props.readonly,
  }
}

/** 卸载视图 */
function destroy(): void {
  queued = false
  diffEpoch++
  diffTask.destroy()
  documentA = undefined
  documentB = undefined
  mergeView?.destroy()
  mergeView = null
  unifiedView?.destroy()
  unifiedView = null
}

/** 使用实际视图内容计算变更，兼容用户已经接受或拒绝过差异块的情况。 */
function updateDocuments(config: ReturnType<typeof configuration>): void {
  if (mergeView) {
    if (config.original !== rendered?.original) {
      const view = mergeView.a
      const changes = documentChange(view.state.doc, view.state.toText(config.original))
      if (changes) view.dispatch({ changes })
    }
    if (config.modified !== rendered?.modified) {
      const view = mergeView.b
      const changes = documentChange(view.state.doc, view.state.toText(config.modified))
      if (changes) view.dispatch({ changes })
    }
  } else if (unifiedView) {
    const view = unifiedView
    const original = getOriginalDoc(view.state)
    const originalChange =
      config.original !== rendered?.original
        ? documentChange(original, view.state.toText(config.original))
        : null
    const changes =
      config.modified !== rendered?.modified
        ? documentChange(view.state.doc, view.state.toText(config.modified))
        : null
    if (originalChange || changes) {
      view.dispatch({
        changes: changes ?? undefined,
        effects: originalChange
          ? originalDocChangeEffect(view.state, ChangeSet.of(originalChange, original.length))
          : undefined,
      })
    }
  }
}

/** 最新配置就绪后更新视图；加载期间保持现有内容可读。 */
async function create(): Promise<void> {
  const request = ++createRequest
  const parent = host.value
  if (!parent) return
  const filename = props.filename
  const language = props.language
  const languageChanged =
    !rendered || filename !== rendered.filename || language !== rendered.language
  const loaded = languageChanged
    ? await loadLanguage(detectLanguage(filename ?? '', language), filename ?? '')
    : languageExtensions
  // 等待期间又来了新请求或组件已卸载：丢弃本次结果，视图归属最新一次请求
  if (request !== createRequest || !host.value) return
  // 等待语言模块时不额外保留每次快速更新的旧正文快照。
  const config = configuration()
  const rebuild =
    !rendered ||
    config.mode !== rendered.mode ||
    (config.mode === 'unified' && config.readonly !== rendered.readonly)
  if (!rebuild) {
    updateDocuments(config)
    if (languageChanged) {
      const effects = languageCompartment.reconfigure(loaded)
      mergeView?.a.dispatch({ effects })
      mergeView?.b.dispatch({ effects })
      unifiedView?.dispatch({ effects })
    }
    if (mergeView && config.readonly !== rendered?.readonly) {
      mergeView.reconfigure({ revertControls: config.readonly ? undefined : 'a-to-b' })
    }
    languageExtensions = loaded
    rendered = config
    refreshDiff()
    return
  }
  destroy()
  languageExtensions = loaded
  const shared: Extension[] = [
    codeEditorTheme,
    syntaxHighlighting(codeHighlightStyle),
    EditorView.editable.of(false),
    languageCompartment.of(loaded),
    EditorView.updateListener.of(scheduleDiff),
  ]
  // 视图可能在建好之前就被卸载（快速切换 props）
  if (request !== createRequest || !host.value) return

  rendered = config
  if (config.mode === 'unified') {
    unifiedView = new EditorView({
      parent,
      state: EditorState.create({
        doc: config.modified,
        extensions: [
          ...shared,
          unifiedMergeView({
            externalDiff: true,
            original: config.original,
            mergeControls: !config.readonly,
            collapseUnchanged: { margin: 3, minSize: 4 },
          }),
        ],
      }),
    })
    refreshDiff()
    return
  }

  mergeView = new MergeView({
    externalDiff: true,
    parent,
    a: { doc: config.original, extensions: [...shared, lineNumbers()] },
    b: { doc: config.modified, extensions: [...shared, lineNumbers()] },
    highlightChanges: true,
    gutter: true,
    collapseUnchanged: { margin: 3, minSize: 4 },
    revertControls: config.readonly ? undefined : 'a-to-b',
  })
  refreshDiff()
}

onMounted(() => void create())
onBeforeUnmount(() => {
  createRequest++
  destroy()
  rendered = null
  languageExtensions = []
})

watch(
  () => [
    props.original,
    props.modified,
    props.mode,
    props.filename,
    props.language,
    props.readonly,
  ],
  () => void create()
)
</script>

<template>
  <div
    class="flex flex-col overflow-hidden rounded-lg border border-border bg-surface dark:border-border-dark dark:bg-surface-dark"
    :style="{ height }"
  >
    <div
      v-if="showStats"
      class="flex shrink-0 items-center gap-3 border-b border-border px-3 py-1.5 text-caption dark:border-border-dark"
    >
      <span class="text-text-muted dark:text-text-muted-dark">{{ modeLabel }}</span>
      <span
        v-if="!calculating && !calculationError && stats.added > 0"
        class="text-success-strong dark:text-success-dark"
      >
        +{{ stats.added }} 行
      </span>
      <span
        v-if="!calculating && !calculationError && stats.removed > 0"
        class="text-danger-strong dark:text-danger-dark"
      >
        -{{ stats.removed }} 行
      </span>
      <span
        v-if="!calculating && !calculationError && stats.same"
        class="text-text-muted dark:text-text-muted-dark"
        >内容一致</span
      >
    </div>
    <div
      v-if="calculating || calculationError"
      role="status"
      class="flex shrink-0 items-center gap-3 px-3 py-1.5 text-caption text-text-muted dark:text-text-muted-dark"
    >
      <span>{{ calculating ? '正在计算差异…' : calculationError }}</span>
      <UiButton v-if="calculating" variant="ghost" size="xs" @click="cancelDiff">取消计算</UiButton>
      <UiButton v-else variant="ghost" size="xs" @click="retryDiff">重新计算</UiButton>
    </div>
    <UiScrollArea as-child axis="vertical" managed>
      <div ref="host" class="min-h-0 flex-1 overflow-hidden" />
    </UiScrollArea>
  </div>
</template>
