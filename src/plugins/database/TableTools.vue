<script setup lang="ts">
/** 单表操作栏：筛选排序、精确统计、主键行编辑；每次异步操作捕获原页签。 */
import { computed, ref, watch } from 'vue'
import { useToolLifecycle } from '@/core/lifecycle'
import { nextRequestId } from './requestId'
import {
  UiButton,
  UiCheckbox,
  UiInput,
  UiModal,
  UiSelect,
  UiToolbar,
  UiCodeEditor,
} from '@/core/ui'
import { queryIpc, tableIpc } from './ipc'
import CsvImport from './CsvImport.vue'
import { ipcScopeArg } from './workspace/useQueryWorkspace'
import type { DbValue, TableChange, TableFilter, TableTarget } from './contracts'
import type { useDatabase } from './useDatabase'

const props = defineProps<{ db: ReturnType<typeof useDatabase> }>()
const db = props.db
const state = computed(() => db.queryState.value)
const busy = ref(false)
const writeRequest = ref('')
const cancelling = ref(false)
const lifecycle = useToolLifecycle('database', { owner: 'database.table-write' })
watch(
  busy,
  (value) => {
    lifecycle.running.value = value
  },
  { flush: 'sync' }
)
async function cancelWrite() {
  if (!writeRequest.value || cancelling.value) return
  cancelling.value = true
  try {
    await queryIpc.cancel(writeRequest.value)
  } catch (cause) {
    error.value = String(cause)
    cancelling.value = false
  }
}
const error = ref('')
const filtersOpen = ref(false)
const sqlPreviewOpen = ref(false)
const sqlPreview = computed(
  () =>
    (state.value.querySql ?? '') +
    '\n\n-- 绑定参数（依次对应占位符，不拼接 SQL）\n' +
    JSON.stringify(state.value.queryParams ?? [], null, 2)
)
const filterRows = ref<Array<{ column: string; operator: TableFilter['operator']; input: string }>>(
  []
)
const sortColumn = ref('')
const descending = ref(false)
const rowAction = ref<TableChange['action'] | ''>('')
const fields = ref<
  Array<{ name: string; kind: DbValue['kind']; value: string; isNull: boolean; included: boolean }>
>([])
const original = ref<Record<string, DbValue>>({})
const target = ref<TableTarget | null>(null)
const targetTab = ref('')
const columnOptions = computed(() =>
  state.value.columns.map((column) => ({ value: column, label: column }))
)
const operators = [
  { value: 'eq', label: '等于' },
  { value: 'ne', label: '不等于' },
  { value: 'lt', label: '小于' },
  { value: 'le', label: '小于等于' },
  { value: 'gt', label: '大于' },
  { value: 'ge', label: '大于等于' },
  { value: 'like', label: 'LIKE' },
  { value: 'in', label: 'IN 列表' },
  { value: 'between', label: '范围（含端点）' },
  { value: 'isNull', label: '为空' },
  { value: 'notNull', label: '非空' },
]
const supported = computed(() =>
  ['mysql', 'polardb', 'postgresql', 'sqlite'].includes(db.activeTabConnection.value?.dbType ?? '')
)
const writable = computed(() => supported.value && !db.activeTabConnection.value?.readonly)
function currentTarget(): TableTarget {
  const ctx = db.activeTabContext.value
  const conn = db.activeTabConnection.value
  if (!conn || !ctx.table) throw new Error('表目标不存在')
  return {
    connId: ctx.connectionId,
    database: ctx.database,
    schema: ipcScopeArg(conn, ctx),
    table: ctx.table,
  }
}
watch(
  () => db.activeTabId.value,
  () => {
    error.value = ''
    filtersOpen.value = false
    if (!busy.value) rowAction.value = ''
    const options = state.value.tableOptions
    filterRows.value =
      options?.filters.map((filter) => ({
        column: filter.column,
        operator: filter.operator,
        input: filter.values?.length
          ? JSON.stringify(filter.values.map((value) => value.value))
          : (filter.value.value ?? ''),
      })) ?? []
    sortColumn.value = options?.sort[0]?.column ?? ''
    descending.value = options?.sort[0]?.descending ?? false
  }
)
function applyFilter() {
  const filters: TableFilter[] = []
  try {
    for (const row of filterRows.value) {
      if (!row.column) throw new Error('请选择筛选列，或移除该条件')
      const filter: TableFilter = {
        column: row.column,
        operator: row.operator,
        value: { kind: 'text', value: row.input },
      }
      if (['in', 'between'].includes(row.operator)) {
        const values: unknown = JSON.parse(row.input)
        if (
          !Array.isArray(values) ||
          !values.every((value) => typeof value === 'string') ||
          values.length < 1 ||
          values.length > 100 ||
          (row.operator === 'between' && values.length !== 2)
        )
          throw new Error(
            '列表请填写 1 至 100 个字符串的 JSON 数组；范围填写两个端点，例如 ["10","20"]'
          )
        filter.values = values.map((value) => ({ kind: 'text', value }))
      }
      filters.push(filter)
    }
  } catch (cause) {
    error.value = String(cause)
    return
  }
  db.patchQueryState({
    tableOptions: {
      filters,
      sort: sortColumn.value ? [{ column: sortColumn.value, descending: descending.value }] : [],
    },
    page: 1,
    exactCount: undefined,
  })
  filtersOpen.value = false
  void db.loadTableData(db.activeTabId.value)
}
async function count() {
  const tabId = db.activeTabId.value
  const source = state.value
  busy.value = true
  error.value = ''
  try {
    writeRequest.value = nextRequestId('table-count')
    cancelling.value = false
    const value = await tableIpc.count(currentTarget(), source.tableOptions, writeRequest.value)
    if (db.tabs.value.some((tab) => tab.id === tabId)) source.exactCount = value
  } catch (cause) {
    error.value = String(cause)
  } finally {
    busy.value = false
    writeRequest.value = ''
  }
}
async function edit(action: TableChange['action']) {
  busy.value = true
  error.value = ''
  const tabId = db.activeTabId.value
  const source = state.value
  try {
    const destination = currentTarget()
    const metadata = await queryIpc.columns(
      destination.connId,
      destination.table,
      destination.schema,
      destination.database
    )
    if (db.activeTabId.value !== tabId) return
    if (action !== 'insert' && !metadata.some((column) => column.key === 'PK'))
      throw new Error('此表没有主键，不能直接更新或删除。')
    const index = Number(source.selectedRow)
    if (action !== 'insert' && (source.selectedRow === '' || !source.values[index]))
      throw new Error('请先选择一行。')
    const cells = action === 'insert' ? [] : source.values[index]
    original.value =
      action === 'insert'
        ? {}
        : Object.fromEntries(source.columns.map((name, i) => [name, cells[i]]))
    fields.value = metadata.map((column) => {
      const cell = original.value[column.name]
      const native = column.dataType.toLowerCase()
      const kind =
        cell?.kind === 'null' || !cell
          ? native.includes('int')
            ? 'integer'
            : native.includes('blob') || native === 'bytea'
              ? 'binary'
              : 'text'
          : cell.kind
      return {
        name: column.name,
        kind,
        value: cell?.value ?? '',
        isNull: cell?.kind === 'null',
        included: action !== 'insert',
      }
    })
    target.value = destination
    targetTab.value = tabId
    rowAction.value = action
  } catch (cause) {
    error.value = String(cause)
  } finally {
    busy.value = false
  }
}
async function submit() {
  if (!target.value || !rowAction.value) return
  const action = rowAction.value
  const tabId = targetTab.value
  const values: Record<string, DbValue> = {}
  if (action !== 'delete') {
    for (const field of fields.value) {
      if (!field.included) continue
      const cell: DbValue = field.isNull
        ? { kind: 'null', value: null }
        : { kind: field.kind, value: field.value }
      const previous = original.value[field.name]
      if (action === 'insert' || cell.kind !== previous?.kind || cell.value !== previous?.value)
        values[field.name] = cell
    }
    if (!Object.keys(values).length) {
      error.value = '没有待提交的修改'
      return
    }
  }
  busy.value = true
  error.value = ''
  try {
    writeRequest.value = nextRequestId('table-write')
    cancelling.value = false
    await tableIpc.apply(
      target.value,
      [{ action, values, original: original.value }],
      writeRequest.value
    )
    rowAction.value = ''
    if (db.tabs.value.some((tab) => tab.id === tabId)) await db.loadTableData(tabId)
  } catch (cause) {
    error.value = String(cause)
  } finally {
    busy.value = false
    writeRequest.value = ''
  }
}
</script>

<template>
  <UiToolbar density="compact" bordered>
    <CsvImport :db="db" />
    <UiButton size="xs" variant="ghost" :disabled="!state.querySql" @click="sqlPreviewOpen = true"
      >查看 SQL</UiButton
    >
    <UiButton
      v-if="writeRequest && !rowAction"
      size="xs"
      variant="secondary"
      :disabled="cancelling"
      @click="cancelWrite"
      >{{ cancelling ? '等待结束…' : '取消统计' }}</UiButton
    >
    <UiButton
      size="xs"
      variant="secondary"
      :disabled="busy || !supported"
      @click="filtersOpen = true"
      >筛选 / 排序</UiButton
    >
    <UiButton size="xs" variant="ghost" :disabled="busy || !supported" @click="count"
      >精确统计</UiButton
    >
    <UiButton size="xs" variant="ghost" :disabled="busy || !writable" @click="edit('insert')"
      >新增行</UiButton
    >
    <UiButton
      size="xs"
      variant="ghost"
      :disabled="busy || !writable || state.selectedRow === ''"
      @click="edit('update')"
      >编辑行</UiButton
    >
    <UiButton
      size="xs"
      variant="ghost"
      :disabled="busy || !writable || state.selectedRow === ''"
      @click="edit('delete')"
      >删除行</UiButton
    >
    <span
      v-if="state.exactCount !== undefined"
      class="text-caption text-secondary dark:text-secondary-dark"
      >精确计数 {{ state.exactCount }} 行</span
    >
    <span
      v-if="state.stableOrder === false"
      class="text-caption text-secondary dark:text-secondary-dark"
      >无主键，翻页顺序可能变化</span
    >
  </UiToolbar>
  <p v-if="error && !rowAction" role="alert" class="px-sm text-caption text-danger">{{ error }}</p>
  <UiModal :open="sqlPreviewOpen" title="当前页查询 SQL 与绑定参数" @close="sqlPreviewOpen = false"
    ><UiCodeEditor :model-value="sqlPreview" language="sql" readonly class="h-[280px]"
  /></UiModal>
  <UiModal
    :open="filtersOpen"
    title="服务端筛选与排序"
    description="筛选作用于整张表；LIKE 使用 % 和 _ 通配符。"
    @close="filtersOpen = false"
  >
    <div class="space-y-sm">
      <div
        v-for="(filter, index) in filterRows"
        :key="index"
        class="grid grid-cols-[1fr_1fr_auto] gap-xs"
      >
        <UiSelect v-model="filter.column" size="xs" :options="columnOptions" aria-label="筛选列" />
        <UiSelect v-model="filter.operator" size="xs" :options="operators" aria-label="筛选条件" />
        <UiButton size="xs" variant="ghost" @click="filterRows.splice(index, 1)">移除</UiButton>
        <UiInput
          v-model="filter.input"
          size="xs"
          class="col-span-3"
          :disabled="['isNull', 'notNull'].includes(filter.operator)"
          aria-label="筛选值"
          :placeholder="
            ['in', 'between'].includes(filter.operator)
              ? '字符串 JSON 数组，例如 [&quot;10&quot;,&quot;20&quot;]'
              : '筛选值'
          "
        />
      </div>
      <UiButton
        size="xs"
        variant="secondary"
        :disabled="filterRows.length >= 20"
        @click="
          filterRows.push({ column: columnOptions[0]?.value ?? '', operator: 'eq', input: '' })
        "
        >添加 AND 条件</UiButton
      >
      <p v-if="error" role="alert" class="text-caption text-danger">{{ error }}</p>
      <div class="grid grid-cols-2 gap-sm">
        <UiSelect
          v-model="sortColumn"
          size="xs"
          :options="[{ value: '', label: '默认主键排序' }, ...columnOptions]"
          aria-label="排序列"
        />
        <UiCheckbox v-model="descending" size="xs" label="降序" />
      </div>
    </div>
    <template #footer
      ><UiButton size="xs" variant="primary" @click="applyFilter">应用</UiButton></template
    >
  </UiModal>
  <UiModal
    :open="!!rowAction"
    :title="rowAction === 'insert' ? '新增行' : rowAction === 'delete' ? '确认删除行' : '编辑行'"
    :description="
      target ? [target.database, target.schema, target.table].filter(Boolean).join(' / ') : ''
    "
    size="lg"
    @close="!busy && (rowAction = '')"
  >
    <p class="mb-sm text-caption text-secondary dark:text-secondary-dark">
      {{
        rowAction === 'insert'
          ? '勾选要写入的列，未勾选列使用数据库默认值。二进制填写十六进制。'
          : rowAction === 'delete'
            ? '将删除这一行；原值变化时整次操作会被拒绝。'
            : 'NULL 与空字符串分别设置；提交时检查原始数据是否被其他客户端修改。'
      }}
    </p>
    <div class="grid gap-xs">
      <div
        v-for="field in fields"
        :key="field.name"
        class="grid grid-cols-[minmax(100px,1fr)_minmax(160px,3fr)_60px] items-center gap-sm"
      >
        <UiCheckbox
          v-if="rowAction === 'insert'"
          v-model="field.included"
          size="xs"
          :label="field.name"
          :disabled="busy"
        />
        <span v-else class="truncate font-mono text-caption" :title="field.name">{{
          field.name
        }}</span>
        <UiInput
          v-model="field.value"
          size="xs"
          :aria-label="field.name"
          :disabled="busy || rowAction === 'delete' || field.isNull || !field.included"
        />
        <UiCheckbox
          v-model="field.isNull"
          size="xs"
          label="NULL"
          :disabled="busy || rowAction === 'delete' || !field.included"
        />
      </div>
    </div>
    <p v-if="error" role="alert" class="mt-sm text-caption text-danger">{{ error }}</p>
    <template #footer>
      <UiButton
        v-if="writeRequest"
        size="xs"
        variant="secondary"
        :disabled="cancelling"
        @click="cancelWrite"
        >{{ cancelling ? '等待回滚…' : '取消写入' }}</UiButton
      >
      <UiButton size="xs" variant="secondary" :disabled="busy" @click="rowAction = ''"
        >取消</UiButton
      >
      <UiButton size="xs" variant="primary" :disabled="busy" @click="submit">{{
        busy ? '提交中…' : rowAction === 'delete' ? '确认删除这一行' : '提交'
      }}</UiButton>
    </template>
  </UiModal>
</template>
