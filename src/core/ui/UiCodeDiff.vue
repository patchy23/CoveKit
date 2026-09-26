<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
/**
 * 代码差异视图（L3 档，基于 `@codemirror/merge`）
 *
 * - `mode="split"`：左右两栏对照（只读），变更行高亮 + 中缝连接线；
 * - `mode="unified"`：单栏内联展示变更块，`readonly=false` 时块上出现接受/拒绝控件；
 * - 顶部按行内容计数给出新增/删除统计（`diff.ts`），视图负责具体变更块的对齐。
 *
 * 两份文本共用异步加载的语言扩展；语言与左右对照的控件配置就地更新，保留阅读现场。
 * 文本、形态或内联控件变化仍重建视图，待独立处理增量文档更新。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { MergeView, unifiedMergeView } from '@codemirror/merge'
import { Compartment, EditorState, type Extension } from '@codemirror/state'
import { EditorView, lineNumbers } from '@codemirror/view'
import { syntaxHighlighting } from '@codemirror/language'
import { codeEditorTheme, codeHighlightStyle } from './editor/theme'
import { detectLanguage, loadLanguage } from './editor/languages'
import { diffStats } from './editor/diff'

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
const stats = computed(() => diffStats(props.original, props.modified))
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
  mergeView?.destroy()
  mergeView = null
  unifiedView?.destroy()
  unifiedView = null
}

/** 最新配置就绪后更新视图；加载期间保持现有内容可读。 */
async function create(): Promise<void> {
  const request = ++createRequest
  const parent = host.value
  if (!parent) return
  const config = configuration()
  const languageChanged =
    !rendered || config.filename !== rendered.filename || config.language !== rendered.language
  const loaded = languageChanged
    ? await loadLanguage(
        detectLanguage(config.filename ?? '', config.language),
        config.filename ?? ''
      )
    : languageExtensions
  // 等待期间又来了新请求或组件已卸载：丢弃本次结果，视图归属最新一次请求
  if (request !== createRequest || !host.value) return
  const rebuild =
    !rendered ||
    config.original !== rendered.original ||
    config.modified !== rendered.modified ||
    config.mode !== rendered.mode ||
    (config.mode === 'unified' && config.readonly !== rendered.readonly)
  if (!rebuild) {
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
    return
  }
  destroy()
  languageExtensions = loaded
  const shared: Extension[] = [
    codeEditorTheme,
    syntaxHighlighting(codeHighlightStyle),
    EditorView.editable.of(false),
    languageCompartment.of(loaded),
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
            original: config.original,
            mergeControls: !config.readonly,
            collapseUnchanged: { margin: 3, minSize: 4 },
          }),
        ],
      }),
    })
    return
  }

  mergeView = new MergeView({
    parent,
    a: { doc: config.original, extensions: [...shared, lineNumbers()] },
    b: { doc: config.modified, extensions: [...shared, lineNumbers()] },
    highlightChanges: true,
    gutter: true,
    collapseUnchanged: { margin: 3, minSize: 4 },
    revertControls: config.readonly ? undefined : 'a-to-b',
  })
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
      <span v-if="stats.added > 0" class="text-success-strong dark:text-success-dark">
        +{{ stats.added }} 行
      </span>
      <span v-if="stats.removed > 0" class="text-danger-strong dark:text-danger-dark">
        -{{ stats.removed }} 行
      </span>
      <span v-if="stats.same" class="text-text-muted dark:text-text-muted-dark">内容一致</span>
    </div>
    <UiScrollArea as-child axis="vertical" managed>
      <div ref="host" class="min-h-0 flex-1 overflow-hidden" />
    </UiScrollArea>
  </div>
</template>
