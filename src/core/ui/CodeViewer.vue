<script setup lang="ts">
/**
 * CodeViewer · CodeMirror 6 只读查看器（JSON/XML 格式化输出）
 * 内置：行号 + 语法高亮（GitHub 配色，随主题切换）+ 折叠箭头（foldGutter）
 * + 缩进参考线 + 点击/全选复制（Ctrl+A 天然支持）。高度由调用方 flex 决定。
 */
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { EditorView, Decoration, ViewPlugin, lineNumbers } from '@codemirror/view'
import { EditorState, type Range } from '@codemirror/state'
import { HighlightStyle, foldGutter, syntaxHighlighting } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'
import { json } from '@codemirror/lang-json'
import { xml } from '@codemirror/lang-xml'

const props = defineProps<{
  doc: string
  lang: 'json' | 'xml' | 'text'
}>()

const host = ref<HTMLElement | null>(null)
let view: EditorView | null = null

/* ── 语法高亮（GitHub 配色，颜色走 CSS 变量随应用主题切换） ── */
const highlight = HighlightStyle.define([
  { tag: t.keyword, color: 'var(--cm-keyword)' },
  { tag: [t.propertyName, t.attributeName], color: 'var(--cm-property)' },
  { tag: [t.string, t.special(t.string)], color: 'var(--cm-string)' },
  { tag: [t.number, t.bool, t.null], color: 'var(--cm-number)' },
  { tag: [t.tagName, t.typeName], color: 'var(--cm-tag)' },
  { tag: [t.angleBracket, t.paren, t.brace, t.bracket, t.separator], color: 'var(--cm-punct)' },
  { tag: t.comment, color: 'var(--cm-comment)', fontStyle: 'italic' },
  { tag: t.operator, color: 'var(--cm-punct)' },
])

/* ── 缩进参考线：每 2 字符一格竖线（VS Code 风格），行 decoration 背景渐变 ── */
const indentGuides = ViewPlugin.fromClass(
  class {
    decorations: ReturnType<typeof Decoration.set>
    constructor(view: EditorView) {
      this.decorations = buildIndentGuides(view)
    }
    update(u: { docChanged: boolean; viewportChanged: boolean; view: EditorView }) {
      if (u.docChanged || u.viewportChanged) this.decorations = buildIndentGuides(u.view)
    }
  },
  { decorations: (v) => v.decorations }
)

function buildIndentGuides(view: EditorView) {
  const decos: Range<Decoration>[] = []
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to;) {
      const line = view.state.doc.lineAt(pos)
      const indent = line.text.match(/^\s*/)?.[0].length ?? 0
      if (indent > 0) {
        decos.push(Decoration.line({ class: 'cm-indent-guides' }).range(line.from))
      }
      pos = line.to + 1
    }
  }
  return Decoration.set(decos)
}

/* ── 初始化 / 内容同步 / 销毁 ── */
function createEditor() {
  const langExt = props.lang === 'json' ? json() : props.lang === 'xml' ? xml() : []
  view = new EditorView({
    parent: host.value!,
    state: EditorState.create({
      doc: props.doc,
      extensions: [
        lineNumbers(),
        foldGutter(),
        syntaxHighlighting(highlight),
        langExt,
        indentGuides,
        EditorView.editable.of(false),
        // 不启用 lineWrapping：长行横向滚动（左右不够时出现横向滚动条）
        EditorView.theme({
          '&': { height: '100%', fontSize: '13px' },
          '.cm-scroller': {
            fontFamily: 'var(--font-mono)',
            lineHeight: '1.5',
            overflow: 'auto',
          },
          '.cm-content': { padding: '10px 0', caretColor: 'transparent' },
          '.cm-line': { padding: '0 12px' },
          '.cm-gutters': {
            background: 'transparent',
            borderRight: '1px solid var(--color-border)',
            color: 'var(--color-text-muted)',
            fontSize: '12px',
          },
          '.cm-foldGutter .cm-gutterElement': { padding: '0 4px' },
          '.cm-foldPlaceholder': {
            background: 'var(--color-tertiary-soft)',
            color: 'var(--color-tertiary-strong)',
            border: 'none',
            borderRadius: '3px',
            margin: '0 2px',
            padding: '0 4px',
          },
        }),
      ],
    }),
  })
}

watch(
  () => props.doc,
  (doc) => {
    if (!view) return
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: doc } })
  }
)

watch(
  () => props.lang,
  () => {
    // 语言切换：重建编辑器（输出区 lang 固定，实际不会触发）
    view?.destroy()
    view = null
    if (host.value) createEditor()
  }
)

onMounted(createEditor)
onUnmounted(() => view?.destroy())
</script>

<template>
  <div
    ref="host"
    class="h-full min-h-0 w-full overflow-hidden rounded-md border border-border bg-surface-muted dark:border-border-dark dark:bg-surface-muted-dark"
  />
</template>
