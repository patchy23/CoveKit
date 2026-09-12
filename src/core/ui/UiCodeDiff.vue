<script setup lang="ts">
/**
 * 代码差异视图（L3 档，基于 `@codemirror/merge`）
 *
 * - `mode="split"`：左右两栏对照（只读），变更行高亮 + 中缝连接线；
 * - `mode="unified"`：单栏内联展示变更块，`readonly=false` 时块上出现接受/拒绝控件；
 * - 顶部给出变更统计（新增/删除行数），与 diff 视图同一套算法（`diff.ts`）。
 *
 * 语言按文件名识别后异步加载，两份文本共用同一个语言扩展；文本或模式变化时重建视图
 * （diff 视图不是高频输入场景，重建比热重配置更简单可靠）。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { MergeView, unifiedMergeView } from '@codemirror/merge'
import { EditorState, type Extension } from '@codemirror/state'
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
/** 差异统计（纯函数，与视图同源） */
const stats = computed(() => diffStats(props.original, props.modified))
/** 形态文案 */
const modeLabel = computed(() => (props.mode === 'unified' ? '内联对比' : '左右对照'))

let mergeView: MergeView | null = null
let unifiedView: EditorView | null = null

/** 卸载视图 */
function destroy(): void {
  mergeView?.destroy()
  mergeView = null
  unifiedView?.destroy()
  unifiedView = null
}

/** 建立视图（语言扩展异步加载完成后再创建，避免闪烁） */
async function create(): Promise<void> {
  const parent = host.value
  if (!parent) return
  destroy()

  const info = detectLanguage(props.filename ?? '', props.language)
  const loaded = await loadLanguage(info, props.filename ?? '')
  const languageExtensions: Extension[] = Array.isArray(loaded) ? loaded : [loaded]
  const shared: Extension[] = [
    codeEditorTheme,
    syntaxHighlighting(codeHighlightStyle),
    EditorView.editable.of(false),
    ...languageExtensions,
  ]
  // 视图可能在建好之前就被卸载（快速切换 props）
  if (!host.value) return

  if (props.mode === 'unified') {
    unifiedView = new EditorView({
      parent,
      state: EditorState.create({
        doc: props.modified,
        extensions: [
          ...shared,
          unifiedMergeView({
            original: props.original,
            mergeControls: !props.readonly,
            collapseUnchanged: { margin: 3, minSize: 4 },
          }),
        ],
      }),
    })
    return
  }

  mergeView = new MergeView({
    parent,
    a: { doc: props.original, extensions: [...shared, lineNumbers()] },
    b: { doc: props.modified, extensions: [...shared, lineNumbers()] },
    highlightChanges: true,
    gutter: true,
    collapseUnchanged: { margin: 3, minSize: 4 },
    revertControls: props.readonly ? undefined : 'a-to-b',
  })
}

onMounted(() => void create())
onBeforeUnmount(destroy)

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
    <div ref="host" class="min-h-0 flex-1 overflow-hidden" />
  </div>
</template>
