<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Compartment, EditorState } from '@codemirror/state'
import { EditorView, placeholder as cmPlaceholder } from '@codemirror/view'
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
import { darkTheme, lightTheme } from './sqlEditorThemes'

export interface SqlEditorTable {
  name: string
  columns: { name: string }[]
}

const props = defineProps<{
  modelValue: string
  placeholder?: string
  tables?: SqlEditorTable[]
  dialect?: string
  resolveColumns?: (table: string) => Promise<string[]>
  onRunStatement?: (sql: string) => void
  onTableClick?: (table: string) => void
}>()
const emit = defineEmits<{ (e: 'update:modelValue', value: string): void }>()

const host = ref<HTMLElement | null>(null)
let view: EditorView | null = null
let observer: MutationObserver | null = null
const themeCompartment = new Compartment()
const langCompartment = new Compartment()

const isDark = () => document.documentElement.dataset.theme === 'dark'

function pickDialect(): typeof MySQL | typeof PostgreSQL | typeof SQLite | undefined {
  const dialect = props.dialect
  if (dialect === 'mysql' || dialect === 'polardb') return MySQL
  if (dialect === 'postgresql' || dialect === 'kingbase' || dialect === 'vastbase')
    return PostgreSQL
  if (dialect === 'sqlite') return SQLite
  return undefined
}

function completionSchema() {
  const namespace: Record<string, string[]> = {}
  for (const table of props.tables ?? []) {
    namespace[table.name] = table.columns.map((column) => column.name)
  }
  return Object.keys(namespace).length ? namespace : undefined
}

function langExtension() {
  const dialect = pickDialect()
  return [
    dialect ? sql({ dialect }) : sql(),
    sqlCompletionExtension(dialect, completionSchema(), props.resolveColumns),
    statementRunGutterExtension(props.onRunStatement),
  ]
}

function knownTableAt(editor: EditorView, event: MouseEvent): string {
  const position = editor.posAtCoords({ x: event.clientX, y: event.clientY })
  if (position == null) return ''
  const word = editor.state.wordAt(position)
  if (!word) return ''
  let { from, to } = word
  const doc = editor.state.doc
  const before = from > 0 ? doc.sliceString(from - 1, from) : ''
  const after = to < doc.length ? doc.sliceString(to, to + 1) : ''
  if ((before === '`' || before === '"') && after === before) {
    from -= 1
    to += 1
  }
  const raw = doc.sliceString(from, to).replace(/^["'`]|["'`]$/g, '')
  return (
    (props.tables ?? []).find((table) => table.name.toLowerCase() === raw.toLowerCase())?.name ?? ''
  )
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
      EditorView.domEventHandlers({
        mousedown: (event, editor) => {
          if (!props.onTableClick || !(event.ctrlKey || event.metaKey) || event.button !== 0)
            return false
          const table = knownTableAt(editor, event)
          if (!table) return false
          event.preventDefault()
          props.onTableClick(table)
          return true
        },
        mousemove: (event, editor) => {
          if (!props.onTableClick) return
          const pointer =
            (event.ctrlKey || event.metaKey) && knownTableAt(editor, event) ? 'pointer' : ''
          if (editor.dom.style.cursor !== pointer) editor.dom.style.cursor = pointer
        },
      }),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) emit('update:modelValue', update.state.doc.toString())
      }),
    ],
  })
}

onMounted(() => {
  if (!host.value) return
  view = new EditorView({ state: createState(), parent: host.value })
  observer = new MutationObserver(() => {
    view?.dispatch({ effects: themeCompartment.reconfigure(isDark() ? darkTheme : lightTheme) })
  })
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
})

onBeforeUnmount(() => {
  observer?.disconnect()
  view?.destroy()
  view = null
})

watch(
  () => props.modelValue,
  (value) => {
    if (!view) return
    const doc = view.state.doc.toString()
    if (value !== doc) view.dispatch({ changes: { from: 0, to: doc.length, insert: value } })
  }
)
watch(
  [() => props.dialect, () => props.tables, () => props.resolveColumns],
  () => view?.dispatch({ effects: langCompartment.reconfigure(langExtension()) }),
  { deep: true }
)

function getSelection(): { from: number; to: number } {
  const selection = view?.state.selection.main
  return { from: selection?.from ?? 0, to: selection?.to ?? 0 }
}
function getDoc(): string {
  return view?.state.doc.toString() ?? props.modelValue
}
function getCursorStatement(): SqlTextRange | null {
  return statementRangeAtCursor(getDoc(), view?.state.selection.main.head ?? 0)
}
function getExecutableSql(): string {
  const { from, to } = getSelection()
  if (from !== to) return getDoc().slice(from, to)
  const statement = getCursorStatement()
  return statement ? statementExecutableSql(statement) : ''
}
function focusEditor() {
  view?.focus()
}

defineExpose({ getSelection, getCursorStatement, getExecutableSql, getDoc, focusEditor })
</script>

<template>
  <div ref="host" class="min-h-0 w-full overflow-hidden" />
</template>
