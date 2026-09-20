<script setup lang="ts">
/** CSV 导入始终捕获原始目标，不随背景页签切换；取消等待服务器回滚。 */
import { ref, computed, watch } from 'vue'
import { useToolLifecycle } from '@/core/lifecycle'
import { open } from '@tauri-apps/plugin-dialog'
import { UiButton, UiModal, UiSelect, UiDataGrid } from '@/core/ui'
import { useUiStore } from '@/stores/ui'
import { csvIpc, queryIpc } from './ipc'
import { ipcScopeArg } from './workspace/useQueryWorkspace'
import { nextRequestId } from './requestId'
import type { CsvPreview, TableTarget } from './contracts'
import type { useDatabase } from './useDatabase'
const props = defineProps<{ db: ReturnType<typeof useDatabase> }>()
const db = props.db
const preview = ref<CsvPreview | null>(null)
const target = ref<TableTarget | null>(null)
const tabId = ref('')
const path = ref('')
const mapping = ref<string[]>([])
const options = ref<Array<{ value: string; label: string }>>([])
const busy = ref(false)
const lifecycle = useToolLifecycle('database', { owner: 'database.csv-import' })
watch(
  busy,
  (value) => {
    lifecycle.running.value = value
  },
  { flush: 'sync' }
)
const loading = ref(false)
const cancelRequested = ref(false)
const requestId = ref('')
const error = ref('')
const rows = computed(
  () =>
    preview.value?.rows.map((row, i) => ({
      id: String(i),
      ...Object.fromEntries(row.map((v, index) => ['c' + index, v])),
    })) ?? []
)
const columns = computed(
  () => preview.value?.columns.map((name, i) => ({ key: 'c' + i, label: name, width: 150 })) ?? []
)
async function choose() {
  const ctx = { ...db.activeTabContext.value }
  const connection = db.activeTabConnection.value
  if (!connection || !ctx.table) return
  const capturedTab = db.activeTabId.value
  const destination = {
    connId: ctx.connectionId,
    database: ctx.database,
    schema: ipcScopeArg(connection, ctx),
    table: ctx.table,
  }
  loading.value = true
  error.value = ''
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'CSV', extensions: ['csv'] }],
    })
    if (typeof selected !== 'string') return
    const [file, metadata] = await Promise.all([
      csvIpc.preview(selected),
      queryIpc.columns(
        destination.connId,
        destination.table,
        destination.schema,
        destination.database
      ),
    ])
    target.value = destination
    tabId.value = capturedTab
    path.value = selected
    options.value = [
      { value: '', label: '跳过此列' },
      ...metadata.map((c) => ({ value: c.name, label: c.name + ' · ' + c.dataType })),
    ]
    mapping.value = file.columns.map((name) => (metadata.some((c) => c.name === name) ? name : ''))
    preview.value = file
  } catch (cause) {
    db.showError(cause)
  } finally {
    loading.value = false
  }
}
async function run() {
  if (!preview.value || !target.value) return
  const mapped = mapping.value
    .map((column, source) => ({ source, column }))
    .filter((item) => item.column)
  if (!mapped.length || new Set(mapped.map((item) => item.column)).size !== mapped.length) {
    error.value = '请选择目标列；同一目标列不能重复映射。'
    return
  }
  busy.value = true
  cancelRequested.value = false
  error.value = ''
  requestId.value = nextRequestId('csv-import')
  try {
    const count = await csvIpc.import(
      target.value,
      path.value,
      preview.value.fingerprint,
      mapped,
      requestId.value
    )
    useUiStore().toast('已导入 ' + count + ' 行')
    preview.value = null
    if (db.tabs.value.some((tab) => tab.id === tabId.value)) await db.loadTableData(tabId.value)
  } catch (cause) {
    error.value = String(cause)
  } finally {
    busy.value = false
    requestId.value = ''
  }
}
async function cancel() {
  if (!requestId.value || cancelRequested.value) return
  cancelRequested.value = true
  try {
    await queryIpc.cancel(requestId.value)
  } catch (cause) {
    error.value = String(cause)
    cancelRequested.value = false
  }
}
</script>
<template>
  <UiButton
    size="xs"
    variant="ghost"
    :disabled="
      loading ||
      busy ||
      db.activeTabConnection.value?.readonly ||
      !['mysql', 'polardb', 'postgresql', 'sqlite'].includes(
        db.activeTabConnection.value?.dbType ?? ''
      )
    "
    @click="choose"
    >导入 CSV</UiButton
  >
  <UiModal
    :open="!!preview"
    title="导入 CSV"
    size="xl"
    :description="
      target ? [target.database, target.schema, target.table].filter(Boolean).join(' / ') : ''
    "
    @close="!busy && (preview = null)"
  >
    <p class="mb-sm text-caption text-secondary dark:text-secondary-dark">
      共 {{ preview?.total }} 行，预览前 20 行。UTF-8，最多 16 MiB / 10000
      行；整批事务提交，任意行失败均回滚。NULL 使用 \N，文本前导反斜线需再加一个反斜线。
    </p>
    <div class="mb-sm grid grid-cols-2 gap-sm">
      <label
        v-for="(column, index) in preview?.columns"
        :key="index"
        class="flex items-center gap-sm text-caption"
      >
        <span class="w-[100px] shrink-0 truncate" :title="column">{{ column }}</span>
        <UiSelect
          v-model="mapping[index]"
          size="xs"
          :options="options"
          :disabled="busy"
          :aria-label="'目标列 ' + column"
          class="min-w-0 flex-1"
        />
      </label>
    </div>
    <UiDataGrid :columns="columns" :rows="rows" row-key="id" height="220px" />
    <p v-if="error" role="alert" class="mt-sm text-caption text-danger">{{ error }}</p>
    <template #footer>
      <UiButton
        v-if="busy"
        size="xs"
        variant="secondary"
        :disabled="cancelRequested"
        @click="cancel"
        >{{ cancelRequested ? '等待执行通道结束…' : '取消导入' }}</UiButton
      >
      <UiButton v-else size="xs" variant="secondary" @click="preview = null">返回</UiButton>
      <UiButton size="xs" variant="primary" :disabled="busy || !preview?.total" @click="run">{{
        busy ? '正在导入…' : '确认导入'
      }}</UiButton>
    </template>
  </UiModal>
</template>
