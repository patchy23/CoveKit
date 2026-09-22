<script setup lang="ts">
/** 网格只暂存修改；手动保存通过单表参数化事务写入，失败保留草稿。 */
import { computed, nextTick, ref, watch } from 'vue'
import {
  UiButton,
  UiContextMenu,
  UiDataGrid,
  UiInput,
  UiModal,
  UiToolbar,
  type UiDataGridColumn,
} from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { queryIpc, tableIpc } from './ipc'
import { nextRequestId } from './requestId'
import { cellTransferText } from './resultText'
import { hasGridChanges, type QueryState } from './workspace/useQueryWorkspace'
import type { DbColumnInfo, DbValue, TableChange } from './contracts'
import type { useDatabase } from './useDatabase'
import CellValueDialog from './CellValueDialog.vue'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
  state: QueryState
  rows: Record<string, unknown>[]
}>()
const { copyText } = useCopy()
const tabId = computed(
  () =>
    Object.keys(props.db.queryStates.value).find(
      (id) => props.db.queryStates.value[id] === props.state
    ) ?? ''
)
function patch(value: Partial<QueryState>) {
  props.db.patchTabQueryState(tabId.value, value)
}
const metadata = ref<DbColumnInfo[]>([])
const metadataError = ref('')
const checking = ref(false)
const selected = ref<{ row: number; column: number } | null>(null)
const editing = ref<{ row: number; column: number } | null>(null)
const input = ref<InstanceType<typeof UiInput> | null>(null)
const detail = ref<{ name: string; value: DbValue } | null>(null)
const menu = ref<{ x: number; y: number } | null>(null)
const discardOpen = ref(false)
let generation = 0
const target = computed(() => props.state.editTarget)
const connection = computed(() =>
  props.db.connections.value.find((c) => c.id === target.value?.connId)
)
const pending = computed(() => hasGridChanges(props.state))
const count = computed(() => Object.keys(props.state.gridEdits ?? {}).length)
const reason = computed(() => {
  if (props.state.gridSaving) return '正在提交事务…'
  if (
    props.state.gridError?.includes('DB_OUTCOME_UNKNOWN') ||
    props.state.gridError?.includes('回滚失败')
  )
    return '提交结果未确认，请先核对数据库；禁止直接重复提交'
  if (props.state.transactionActive)
    return '当前 SQL 页有活动事务，请先提交或回滚，再重新查询后编辑'
  if (!target.value) return '只读结果：仅支持可确定来源的单表直接查询'
  if (!connection.value || connection.value.status !== 'online') return '连接已断开'
  if (connection.value.readonly) return '当前连接为只读'
  if (!['mysql', 'polardb', 'postgresql', 'sqlite'].includes(connection.value.dbType))
    return '当前数据库暂不支持网格事务编辑'
  if (checking.value) return '正在核对主键和列结构…'
  if (metadataError.value) return metadataError.value
  if (!metadata.value.some((c) => c.key === 'PK')) return '只读结果：表没有主键，无法安全定位原行'
  if (
    new Set(props.state.columns).size !== metadata.value.length ||
    props.state.columns.length !== metadata.value.length ||
    metadata.value.some((c) => !props.state.columns.includes(c.name))
  )
    return '只读结果：须查询完整列且不使用列别名'
  if (
    props.state.values.length !== props.state.rows.length ||
    props.state.values.some((row) => row.length !== props.state.columns.length)
  )
    return '只读结果：缺少完整原值'
  return ''
})
watch(
  [() => props.state, () => props.state.editTarget],
  async () => {
    const request = ++generation
    selected.value = null
    editing.value = null
    detail.value = null
    menu.value = null
    discardOpen.value = false
    metadata.value = []
    metadataError.value = ''
    const destination = target.value
    if (!destination) {
      checking.value = false
      return
    }
    checking.value = true
    try {
      const columns = await queryIpc.columns(
        destination.connId,
        destination.table,
        destination.schema,
        destination.database
      )
      if (generation === request) metadata.value = columns
    } catch (error) {
      if (generation === request) metadataError.value = `列结构读取失败：${String(error)}`
    } finally {
      if (generation === request) checking.value = false
    }
  },
  { immediate: true }
)

type CellEvent = { row: Record<string, unknown>; column: UiDataGridColumn }
function selectCell(event: CellEvent) {
  selected.value = { row: Number(event.row.__row), column: Number(event.column.key.slice(1)) }
  patch({ selectedRow: String(event.row.__row) })
}
function valueAt(row: number, column: number): DbValue {
  return (
    props.state.gridEdits?.[row]?.[props.state.columns[column]] ??
    props.state.values[row]?.[column] ?? {
      kind: 'text',
      value: props.state.rows[row]?.[column] ?? '',
    }
  )
}
function display(row: number, column: number) {
  const value = valueAt(row, column)
  return value.kind === 'null'
    ? 'NULL'
    : value.value === ''
      ? '(空字符串)'
      : value.kind === 'binary'
        ? `0x${value.value}`
        : value.value
}
async function start(event?: CellEvent) {
  if (event) selectCell(event)
  if (!selected.value || reason.value) return
  const { row, column } = selected.value
  if ((valueAt(row, column).value?.length ?? 0) > 1024 * 1024) {
    patch({ gridError: '单元格超过 1 MiB，请使用 SQL 修改' })
    return
  }
  editing.value = { row, column }
  await nextTick()
  input.value?.focus()
  input.value?.select()
}
function put(row: number, column: number, value: DbValue) {
  const original = props.state.values[row]?.[column]
  if (!original) return
  const name = props.state.columns[column]
  const edits = { ...props.state.gridEdits }
  const cells = { ...edits[row] }
  if (value.kind === original.kind && value.value === original.value) delete cells[name]
  else cells[name] = value
  if (Object.keys(cells).length) edits[row] = cells
  else delete edits[row]
  patch({ gridEdits: edits, gridMessage: '' })
}
function change(text: string | number) {
  if (!editing.value) return
  if (String(text).length > 1024 * 1024) {
    patch({ gridError: '单元格超过 1 MiB，请使用 SQL 修改' })
    return
  }
  const { row, column } = editing.value
  const previous = valueAt(row, column)
  const native =
    metadata.value.find((c) => c.name === props.state.columns[column])?.dataType.toLowerCase() ?? ''
  const kind =
    previous.kind !== 'null'
      ? previous.kind
      : native.includes('int')
        ? 'integer'
        : native.includes('blob') || native === 'bytea'
          ? 'binary'
          : native.includes('bool')
            ? 'boolean'
            : 'text'
  put(row, column, { kind, value: String(text) })
}
function setNull() {
  if (reason.value || !selected.value) return
  const { row, column } = selected.value
  put(row, column, { kind: 'null', value: null })
  editing.value = null
}
function showDetail() {
  if (!selected.value) return
  const { row, column } = selected.value
  detail.value = { name: props.state.columns[column], value: valueAt(row, column) }
}
function copyCell() {
  if (selected.value)
    void copyText(cellTransferText(valueAt(selected.value.row, selected.value.column)))
}
function context(event: CellEvent, mouse: MouseEvent) {
  mouse.preventDefault()
  selectCell(event)
  menu.value = { x: mouse.clientX, y: mouse.clientY }
}
const menuItems = computed(() => [
  ...(!reason.value
    ? [
        {
          label: '编辑单元格',
          onClick: () => {
            void start()
          },
        },
        { label: '设为 NULL', onClick: setNull },
      ]
    : []),
  { label: '复制单元格', onClick: copyCell },
  { label: '查看完整值', onClick: showDetail },
])
async function save() {
  if (reason.value || !pending.value || !target.value) return
  const source = props.state
  const savedTabId = tabId.value
  const update = (value: Partial<QueryState>) => props.db.patchTabQueryState(savedTabId, value)
  const destination = { ...target.value }
  const changes: TableChange[] = Object.entries(source.gridEdits ?? {}).map(([row, values]) => ({
    action: 'update',
    values: { ...values },
    original: Object.fromEntries(
      source.columns.map((name, index) => [name, source.values[Number(row)][index]])
    ),
  }))
  update({ gridSaving: true, gridError: '' })
  editing.value = null
  try {
    const affected = await tableIpc.apply(destination, changes, nextRequestId('grid-save'))
    const values = source.values.map((row, index) =>
      row.map((value, column) => source.gridEdits?.[index]?.[source.columns[column]] ?? value)
    )
    const rows = values.map((row) =>
      row.map((value) =>
        value.kind === 'null'
          ? 'NULL'
          : value.kind === 'binary'
            ? '0x' + value.value
            : (value.value ?? '')
      )
    )
    // 保存后需重新读取触发器等产生的最终值，不能继续使用旧并发快照写入。
    update({
      values,
      rows,
      gridEdits: {},
      editTarget: null,
      gridMessage: `已提交 ${affected} 行，请刷新或重新查询以获取数据库最新值`,
    })
    if (props.db.tabs.value.find((tab) => tab.id === savedTabId)?.kind === 'data') {
      update({ gridSaving: false })
      await props.db.loadTableData(savedTabId)
      update({ gridMessage: `已提交 ${affected} 行` })
    }
  } catch (error) {
    const message = String(error)
    const uncertain = message.includes('DB_OUTCOME_UNKNOWN') || message.includes('回滚失败')
    update({ gridError: message, editTarget: uncertain ? null : source.editTarget })
  } finally {
    update({ gridSaving: false })
  }
}
function discard() {
  patch({ gridEdits: {}, gridError: '', gridMessage: '已放弃本地编辑草稿，请刷新核对数据库当前值' })
  editing.value = null
  discardOpen.value = false
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <UiToolbar density="compact" bordered>
      <UiButton size="xs" :disabled="!!reason || !selected" @click="start()">编辑单元格</UiButton>
      <UiButton size="xs" :disabled="!!reason || !selected" @click="setNull">设为 NULL</UiButton>
      <UiButton size="xs" variant="ghost" :disabled="!selected" @click="showDetail"
        >查看完整值</UiButton
      >
      <UiButton size="xs" variant="ghost" :disabled="!selected" @click="copyCell"
        >复制单元格</UiButton
      >
      <template #trailing>
        <span v-if="target" class="text-caption text-text-muted"
          >{{ connection?.label }} / {{ target.schema || target.database }}.{{ target.table }}</span
        >
        <span class="text-caption text-text-muted">{{
          pending ? `${count} 行待保存` : '双击编辑'
        }}</span>
        <UiButton size="xs" variant="primary" :disabled="!pending || !!reason" @click="save">{{
          state.gridSaving ? '提交中…' : '保存并提交'
        }}</UiButton>
        <UiButton size="xs" :disabled="!pending || state.gridSaving" @click="discardOpen = true"
          >放弃修改</UiButton
        >
      </template>
    </UiToolbar>
    <p
      v-if="state.gridError || state.gridMessage || reason"
      role="status"
      class="px-[6px] py-[2px] text-caption"
      :class="state.gridError ? 'text-danger-strong' : 'text-text-muted'"
    >
      {{ state.gridError || state.gridMessage || reason }}
    </p>
    <UiDataGrid
      :model-value="state.selectedRow"
      :columns="db.tableColumns.value"
      :rows="rows"
      row-key="__row"
      height="100%"
      class="min-h-0 flex-1"
      @update:model-value="patch({ selectedRow: String($event) })"
      @cell="start"
      @cell-contextmenu="context"
    >
      <template #cell="{ row, column }">
        <UiInput
          v-if="
            editing?.row === Number(row.__row) && editing?.column === Number(column.key.slice(1))
          "
          ref="input"
          size="xs"
          class="w-full min-w-0 font-data"
          :aria-label="`编辑 ${column.label}`"
          :model-value="valueAt(editing.row, editing.column).value ?? ''"
          @update:model-value="change"
          @blur="editing = null"
          @dblclick.stop
        />
        <span
          v-else
          class="block min-h-[20px]"
          :class="
            state.gridEdits?.[Number(row.__row)]?.[column.label]
              ? 'bg-warning-soft text-warning-strong dark:bg-warning-soft-dark'
              : ''
          "
          @click="selectCell({ row, column })"
          >{{ display(Number(row.__row), Number(column.key.slice(1))) }}</span
        >
      </template>
      <template #empty><slot name="empty">暂无数据</slot></template>
    </UiDataGrid>
    <UiContextMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      size="sm"
      @close="menu = null"
    />
    <CellValueDialog :detail="detail" @close="detail = null" />
    <UiModal :open="discardOpen" title="放弃未保存修改" size="sm" @close="discardOpen = false">
      <p class="text-body-sm">
        放弃当前结果中 {{ count }} 行编辑草稿？此操作只清除本地草稿，不撤销已提交的数据库修改。
      </p>
      <template #footer
        ><UiButton size="xs" @click="discardOpen = false">返回</UiButton
        ><UiButton size="xs" variant="primary" @click="discard">放弃修改</UiButton></template
      >
    </UiModal>
  </div>
</template>
