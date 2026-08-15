<script setup lang="ts">
/**
 * SqlEditor · CodeMirror 6 SQL 编辑器
 * 高亮（方言语法）+ 补全（关键字/表/列/snippet 模板）+ 行号 + 语句折叠 + 当前语句框选 + 查找替换。
 * 主题跟随应用 data-theme（浅色/深色 token 复用项目 CSS 变量）；
 * 编辑类按键：Ctrl+F 查找替换、Ctrl+/ 注释、Tab 缩进、Ctrl+Z 撤销（执行/保存一律走工具栏按钮）。
 * 选区/光标/语句范围通过 defineExpose 暴露（执行"选中段/当前语句/全部"语义）。
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Compartment, EditorState } from '@codemirror/state'
import { placeholder as cmPlaceholder } from '@codemirror/view'
import { EditorView } from '@codemirror/view'
import { MySQL, PostgreSQL, SQLite, sql } from '@codemirror/lang-sql'
import {
  sqlCompletionExtension,
  sqlEditorBasics,
  statementRunGutterExtension,
} from './sqlEditorExtensions'
import {
  statementRangeAtCursor,
  statementExecutableSql,
  type SqlTextRange,
} from './sqlStatementRanges'

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
  /** 表名 → 列名异步解析（表. 后补全列；未提供则仅 schema 已有列） */
  resolveColumns?: (table: string) => Promise<string[]>
  /** 语句行前 ▶ 点击：执行该条语句 */
  onRunStatement?: (sql: string) => void
}>()

const emit = defineEmits<{ (e: 'update:modelValue', value: string): void }>()

const host = ref<HTMLElement | null>(null)
let view: EditorView | null = null
let observer: MutationObserver | null = null
const themeCompartment = new Compartment()
const langCompartment = new Compartment()

function isDark(): boolean {
  return document.documentElement.dataset.theme === 'dark'
}

/** 浅色主题（token 复用项目 CSS 变量） */
const lightTheme = EditorView.theme({
  '&': {
    backgroundColor: 'var(--color-surface-muted)',
    color: 'var(--color-primary)',
    fontSize: '13px',
    height: '100%',
  },
  '.cm-scroller': { overflow: 'auto' },
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
  '.cm-lineNumbers .cm-gutterElement': { padding: '0 6px 0 2px', minWidth: '26px' },
  '.cm-foldGutter': { minWidth: '14px' },
  '.cm-foldGutter .cm-gutterElement': {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    cursor: 'pointer',
  },
  '.cm-fold-marker': {
    display: 'inline-flex',
    color: 'var(--color-text-muted)',
    opacity: '0.55',
    transition: 'opacity 0.12s',
  },
  '.cm-fold-marker:hover': { opacity: '1', color: 'var(--color-secondary)' },
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
  '.cm-run-statement-gutter': { minWidth: '24px' },
  '.cm-run-statement-gutter .cm-gutterElement': {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    minWidth: '24px',
    padding: '0 2px',
  },
  '.cm-run-statement-btn': {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '20px',
    height: '20px',
    margin: '0',
    padding: '0',
    border: 'none',
    borderRadius: '4px',
    background: 'transparent',
    color: 'var(--color-success-strong)',
    cursor: 'pointer',
    opacity: '0',
    transition: 'opacity 0.12s, background-color 0.12s',
  },
  '.cm-run-statement-btn:hover': { background: 'var(--color-success-soft)', opacity: '1' },
  '&.cm-editor:hover .cm-run-statement-btn': { opacity: '0.55' },
  '&.cm-editor:hover .cm-run-statement-btn:hover': { opacity: '1' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: 'var(--color-tertiary-soft)',
    color: 'var(--color-tertiary-strong)',
  },
  '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--font-mono)' },
  '.cm-searchMatch': { backgroundColor: 'var(--color-tertiary-soft)' },
  '.cm-selectionMatch': { backgroundColor: 'var(--color-tertiary-soft)' },
})

/** 深色主题 */
const darkTheme = EditorView.theme(
  {
    '&': {
      backgroundColor: 'var(--color-surface-muted-dark)',
      color: 'var(--color-primary-dark)',
      fontSize: '13px',
      height: '100%',
    },
    '.cm-scroller': { overflow: 'auto' },
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
    '.cm-lineNumbers .cm-gutterElement': { padding: '0 6px 0 2px', minWidth: '26px' },
    '.cm-foldGutter': { minWidth: '14px' },
    '.cm-foldGutter .cm-gutterElement': {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      cursor: 'pointer',
    },
    '.cm-fold-marker': {
      display: 'inline-flex',
      color: 'var(--color-text-muted-dark)',
      opacity: '0.55',
      transition: 'opacity 0.12s',
    },
    '.cm-fold-marker:hover': { opacity: '1', color: 'var(--color-secondary-dark)' },
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
    '.cm-run-statement-btn': {
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: '20px',
      height: '20px',
      margin: '0',
      padding: '0',
      border: 'none',
      borderRadius: '4px',
      background: 'transparent',
      color: 'var(--color-success-dark)',
      cursor: 'pointer',
      opacity: '0',
      transition: 'opacity 0.12s, background-color 0.12s',
    },
    '.cm-run-statement-btn:hover': { background: 'var(--color-success-soft-dark)', opacity: '1' },
    '&.cm-editor:hover .cm-run-statement-btn': { opacity: '0.55' },
    '&.cm-editor:hover .cm-run-statement-btn:hover': { opacity: '1' },
    '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
      backgroundColor: 'var(--color-tertiary-soft-dark)',
      color: 'var(--color-tertiary-dark)',
    },
    '.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--font-mono)' },
    '.cm-searchMatch': { backgroundColor: 'var(--color-tertiary-soft-dark)' },
    '.cm-selectionMatch': { backgroundColor: 'var(--color-tertiary-soft-dark)' },
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

function langExtension() {
  const dialect = pickDialect()
  const lang = dialect ? sql({ dialect }) : sql()
  return [
    lang,
    sqlCompletionExtension(dialect, completionSchema(), props.resolveColumns),
    statementRunGutterExtension(props.onRunStatement),
  ]
}

function createState(): EditorState {
  return EditorState.create({
    doc: props.modelValue,
    extensions: [
      langCompartment.of(langExtension()),
      ...sqlEditorBasics(),
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

/** 元数据/方言/列解析变化（连接切换/对象加载完成）→ 刷新语言与补全 */
watch(
  [() => props.dialect, () => props.tables, () => props.resolveColumns],
  () => {
    if (!view) return
    view.dispatch({ effects: langCompartment.reconfigure(langExtension()) })
  },
  { deep: true }
)

/** 选区/光标（供上层计算"选中段/当前语句"执行范围） */
function getSelection(): { from: number; to: number } {
  const sel = view?.state.selection.main
  return { from: sel?.from ?? 0, to: sel?.to ?? 0 }
}

/** 光标所在完整语句范围（无选区时执行语义） */
function getCursorStatement(): SqlTextRange | null {
  const doc = getDoc()
  const head = view?.state.selection.main.head ?? 0
  return statementRangeAtCursor(doc, head)
}

/** 可执行文本：有选区取选区；否则取光标所在语句（去除结尾分号） */
function getExecutableSql(): string {
  const { from, to } = getSelection()
  if (from !== to) return getDoc().slice(from, to)
  const stmt = getCursorStatement()
  return stmt ? statementExecutableSql(stmt) : ''
}

/** 当前文档全文（供上层兜底） */
function getDoc(): string {
  return view?.state.doc.toString() ?? props.modelValue
}

function focusEditor() {
  view?.focus()
}

defineExpose({ getSelection, getCursorStatement, getExecutableSql, getDoc, focusEditor })
</script>

<template>
  <div ref="host" class="min-h-0 w-full overflow-hidden" />
</template>
