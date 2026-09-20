<script setup lang="ts">
/**
 * SQL 编辑器（数据库插件薄封装）
 *
 * 底座为 core 的 `UiCodeEditor`：方言语法高亮、SQL 补全（snippet 模板 / 表列异步 / 关键字 / schema）、
 * 每条语句起始行的 ▶ 执行按钮、Ctrl(⌘)+点击表名跳结构。本组件只做两件事：
 * 把插件领域能力接进编辑器，以及按「有选中执行选中段、否则执行光标所在语句」给出可执行 SQL。
 */
import { computed, ref, nextTick, watch } from 'vue'
import {
  MySQL,
  PLSQL,
  PostgreSQL,
  SQLite,
  sql,
  type SQLDialect,
  type SQLNamespace,
} from '@codemirror/lang-sql'
import type { CompletionSource } from '@codemirror/autocomplete'
import { Prec, type Extension } from '@codemirror/state'
import { keymap } from '@codemirror/view'
import { UiCodeEditor } from '@/core/ui'
import {
  sqlCompletionSources,
  statementRunGutterExtension,
  tableNavigationExtension,
} from './sqlEditorExtensions'
import {
  statementExecutableSql,
  statementRangeAtCursor,
  type SqlTextRange,
} from './sqlStatementRanges'

export interface SqlEditorTable {
  name: string
  columns: { name: string }[]
}

const props = defineProps<{
  selection?: { from: number; to: number }
  documentKey?: string
  documentKeys?: string[]
  modelValue: string
  placeholder?: string
  tables?: SqlEditorTable[]
  dialect?: string
  resolveColumns?: (table: string) => Promise<string[]>
  onRunStatement?: (sql: string) => void
  onTableClick?: (table: string) => void
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'save'): void
  (e: 'selection', value: { from: number; to: number }): void
}>()

/** UiCodeEditor 实例引用 */
const editor = ref<InstanceType<typeof UiCodeEditor> | null>(null)

watch(
  () => props.documentKey,
  async () => {
    const key = props.documentKey
    const selection = props.selection
    await nextTick()
    if (key === props.documentKey && selection)
      editor.value?.setCursor(selection.from, selection.to)
  },
  { immediate: true, flush: 'post' }
)
function reportSelection() {
  const range = editor.value?.getCursor()
  if (range) emit('selection', { from: range.from, to: range.to })
}

/** 方言对象：MySQL 系（mysql / polardb）、PostgreSQL 系（postgresql / kingbase / vastbase）、SQLite */
const dialect = computed<SQLDialect | undefined>(() => {
  const name = props.dialect
  if (name === 'mysql' || name === 'polardb') return MySQL
  if (name === 'postgresql' || name === 'kingbase' || name === 'vastbase') return PostgreSQL
  if (name === 'sqlite') return SQLite
  if (name === 'oracle') return PLSQL
  return undefined
})

/** schema 命名空间：表名 → 列名（由当前连接的库结构生成） */
const schema = computed<SQLNamespace | undefined>(() => {
  const namespace: Record<string, string[]> = {}
  for (const table of props.tables ?? []) {
    namespace[table.name] = table.columns.map((column) => column.name)
  }
  return Object.keys(namespace).length ? namespace : undefined
})

/**
 * 语言扩展：方言高亮
 *
 * UiCodeEditor 的 `language="sql"` 只能给出标准 SQL，方言需由插件自行构造后注入。
 */
const languageExtension = computed<Extension>(() => sql({ dialect: dialect.value }))

/** SQL 补全源（snippet / 表列异步 / 关键字 / schema） */
const completionSources = computed<CompletionSource[]>(() =>
  sqlCompletionSources(dialect.value, schema.value, props.resolveColumns)
)

/** 插件专用扩展：语句运行按钮 + Ctrl(⌘)+点击表名 */
const extraExtensions = computed<Extension[]>(() => [
  Prec.highest(
    keymap.of([
      {
        key: 'Mod-Enter',
        run: () => {
          props.onRunStatement?.(getExecutableSql())
          return true
        },
      },
      {
        key: 'Mod-Shift-Enter',
        run: () => {
          props.onRunStatement?.(getDoc())
          return true
        },
      },
    ])
  ),
  ...(props.onRunStatement
    ? [statementRunGutterExtension(props.onRunStatement, props.dialect)]
    : []),
  ...(props.onTableClick
    ? [tableNavigationExtension(() => props.tables ?? [], props.onTableClick)]
    : []),
])

/** 当前文档内容 */
function getDoc(): string {
  return editor.value?.getValue() ?? props.modelValue
}

/** 当前选区偏移 */
function getSelection(): { from: number; to: number } {
  const cursor = editor.value?.getCursor() ?? { from: 0, to: 0, head: 0 }
  return { from: cursor.from, to: cursor.to }
}

/** 光标所在语句范围 */
function getCursorStatement(): SqlTextRange | null {
  const cursor = editor.value?.getCursor()
  return statementRangeAtCursor(getDoc(), cursor?.head ?? 0, props.dialect)
}

/** 可执行 SQL：有选中执行选中段，否则执行光标所在语句 */
function getExecutableSql(): string {
  const { from, to } = getSelection()
  if (from !== to) return getDoc().slice(from, to)
  const statement = getCursorStatement()
  return statement ? statementExecutableSql(statement) : ''
}

/** 聚焦编辑器 */
function focusEditor(): void {
  editor.value?.focus()
}

defineExpose({ getSelection, getCursorStatement, getExecutableSql, getDoc, focusEditor })
</script>

<template>
  <UiCodeEditor
    ref="editor"
    :document-key="documentKey"
    :document-keys="documentKeys"
    :model-value="modelValue"
    language="sql"
    :language-extension="languageExtension"
    :completion-sources="completionSources"
    :extra-extensions="extraExtensions"
    :placeholder="placeholder"
    line-wrapping
    class="min-h-0 flex-1 !rounded-none !border-0"
    @cursor="reportSelection"
    @save="emit('save')"
    @update:model-value="emit('update:modelValue', $event)"
  />
</template>
