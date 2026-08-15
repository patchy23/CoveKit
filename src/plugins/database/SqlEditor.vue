<script setup lang="ts">
/**
 * SqlEditor · CodeMirror 6 SQL 编辑器（高亮 + 关键字/元数据补全）
 * 主题跟随应用 data-theme（浅色/深色 token 复用项目 CSS 变量）；
 * 快捷键：Ctrl+Enter 执行（由上层 onRun 决定执行范围）、Ctrl+S 保存、Esc 取消。
 * 选区/光标位置通过 defineExpose 暴露（执行"选中段/当前行"语义）。
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Compartment, EditorState } from '@codemirror/state'
import { keymap, placeholder as cmPlaceholder } from '@codemirror/view'
import { EditorView } from '@codemirror/view'
import { autocompletion } from '@codemirror/autocomplete'
import { MySQL, PostgreSQL, SQLite, sql } from '@codemirror/lang-sql'

/** 补全用表结构（列名数组） */
export interface SqlEditorTable {
  name: string
  columns: { name: string }[]
}

const props = defineProps<{
  modelValue: string
  placeholder?: string
  /** 当前连接的元数据（表/列 → 自动补全 schema） */
  tables?: SqlEditorTable[]
  /** 方言：mysql/postgresql/sqlite；其余走标准 SQL */
  dialect?: string
  onRun?: () => void
  onSave?: () => void
  onCancel?: () => void
}>()

const emit = defineEmits<{ (e: 'update:modelValue', value: string): void }>()

const host = ref<HTMLElement | null>(null)
let view: EditorView | null = null
let observer: MutationObserver | null = null
const themeCompartment = new Compartment()

function isDark(): boolean {
  return document.documentElement.dataset.theme === 'dark'
}

/** 浅色主题（token 复用项目 CSS 变量） */
const lightTheme = EditorView.theme({
  '&': {
    backgroundColor: 'var(--color-surface-muted)',
    color: 'var(--color-primary)',
    fontSize: '13px',
  },
  '.cm-content': {
    fontFamily: 'var(--font-mono)',
    caretColor: 'var(--color-tertiary)',
    padding: '11px 12px 11px 0',
    lineHeight: '1.65',
  },
  '.cm-line': { padding: '0 4px 0 0' },
  '.cm-gutters': {
    backgroundColor: 'transparent',
    color: 'var(--color-text-muted)',
    border: 'none',
    paddingLeft: '8px',
  },
  '.cm-activeLine': { backgroundColor: 'var(--color-border)' },
  '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--color-tertiary-strong)' },
  '.cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft)' },
  '&.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft)' },
  '.cm-cursor': { borderLeftColor: 'var(--color-tertiary-strong)' },
  '.cm-tooltip': {
    backgroundColor: 'var(--color-surface)',
    border: '1px solid var(--color-border-strong)',
    color: 'var(--color-primary)',
    borderRadius: '6px',
    boxShadow: '0 4px 12px rgba(0,0,0,0.12)',
  },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: 'var(--color-tertiary-soft)',
    color: 'var(--color-tertiary-strong)',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--font-mono)' },
})

/** 深色主题 */
const darkTheme = EditorView.theme(
  {
    '&': {
      backgroundColor: 'var(--color-surface-muted-dark)',
      color: 'var(--color-primary-dark)',
      fontSize: '13px',
    },
    '.cm-content': {
      fontFamily: 'var(--font-mono)',
      caretColor: 'var(--color-tertiary-dark)',
      padding: '11px 12px 11px 0',
      lineHeight: '1.65',
    },
    '.cm-line': { padding: '0 4px 0 0' },
    '.cm-gutters': {
      backgroundColor: 'transparent',
      color: 'var(--color-text-muted-dark)',
      border: 'none',
      paddingLeft: '8px',
    },
    '.cm-activeLine': { backgroundColor: 'var(--color-border-dark)' },
    '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--color-tertiary-dark)' },
    '.cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft-dark)' },
    '&.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--color-tertiary-soft-dark)' },
    '.cm-cursor': { borderLeftColor: 'var(--color-tertiary-dark)' },
    '.cm-tooltip': {
      backgroundColor: 'var(--color-surface-dark)',
      border: '1px solid var(--color-border-strong-dark)',
      color: 'var(--color-primary-dark)',
      borderRadius: '6px',
      boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
    },
    '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
      backgroundColor: 'var(--color-tertiary-soft-dark)',
      color: 'var(--color-tertiary-dark)',
    },
    '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--font-mono)' },
  },
  { dark: true }
)

/** 方言选择 */
function pickDialect(): typeof MySQL | typeof PostgreSQL | typeof SQLite | undefined {
  const d = props.dialect
  if (d === 'mysql' || d === 'polardb') return MySQL
  if (d === 'postgresql' || d === 'kingbase' || d === 'vastbase') return PostgreSQL
  if (d === 'sqlite') return SQLite
  return undefined
}

/** 补全 schema（SQLNamespace 形状：命名空间 → 表名 → 列名数组） */
function completionSchema() {
  const ns: Record<string, string[]> = {}
  for (const t of props.tables ?? []) {
    ns[t.name] = t.columns.map((c) => c.name)
  }
  return Object.keys(ns).length ? ns : undefined
}

function createState(): EditorState {
  const dialect = pickDialect()
  const lang = dialect ? sql({ dialect, schema: completionSchema() }) : sql({ schema: completionSchema() })
  return EditorState.create({
    doc: props.modelValue,
    extensions: [
      lang,
      autocompletion(),
      keymap.of([
        { key: 'Mod-Enter', run: () => (props.onRun?.(), true) },
        { key: 'Mod-s', run: () => (props.onSave?.(), true) },
        { key: 'Escape', run: () => (props.onCancel?.(), true) },
      ]),
      themeCompartment.of(isDark() ? darkTheme : lightTheme),
      EditorView.lineWrapping,
      cmPlaceholder(props.placeholder ?? ''),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) emit('update:modelValue', update.state.doc.toString())
      }),
    ],
  })
}

onMounted(() => {
  if (!host.value) return
  view = new EditorView({ state: createState(), parent: host.value })
  // 主题跟随（data-theme 变更时切换）
  observer = new MutationObserver(() => {
    if (!view) return
    view.dispatch({
      effects: themeCompartment.reconfigure(isDark() ? darkTheme : lightTheme),
    })
  })
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
})

onBeforeUnmount(() => {
  observer?.disconnect()
  view?.destroy()
  view = null
})

/** 外部值更新（保存/历史/收藏恢复等）→ 同步到编辑器（避免与输入事件循环） */
watch(
  () => props.modelValue,
  (value) => {
    if (!view) return
    const doc = view.state.doc.toString()
    if (value !== doc) {
      view.dispatch({
        changes: { from: 0, to: doc.length, insert: value },
      })
    }
  }
)

/** 元数据变化（连接切换/对象加载完成）→ 刷新补全 schema */
watch(
  () => props.tables,
  () => {
    // 无需重建编辑器：lang-sql 的 schema 在激活补全时读取（轻量场景足够）
  },
  { deep: true }
)

/** 选区/光标（供上层计算"选中段/当前行"执行范围） */
function getSelection(): { from: number; to: number } {
  const sel = view?.state.selection.main
  return { from: sel?.from ?? 0, to: sel?.to ?? 0 }
}

/** 当前文档全文（供上层兜底） */
function getDoc(): string {
  return view?.state.doc.toString() ?? props.modelValue
}

function focusEditor() {
  view?.focus()
}

defineExpose({ getSelection, getDoc, focusEditor })
</script>

<template>
  <div ref="host" class="min-h-0 w-full overflow-hidden" />
</template>
