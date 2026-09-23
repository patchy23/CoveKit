<script setup lang="ts">
import { computed, ref, onUnmounted } from 'vue'
import { open, save } from '@tauri-apps/plugin-dialog'
import {
  UiModal,
  UiButton,
  UiTextarea,
  UiSelect,
  UiField,
  UiAlert,
  UiCheckbox,
  UiTable,
  UiTableCell,
  UiScrollArea,
  UiToolbar,
} from '@/core/ui'
import { useToolLifecycle } from '@/core/lifecycle'
import { ipc } from '../ipc'
import type { ServerProfile, SshGroup } from '../contracts'
import {
  fields,
  parseTable,
  mapHeaders,
  previewRows,
  csvText,
  serverCsvTemplate,
  type Mapping,
} from './serverCsv'
import { importServers, type PasswordMode, type ImportResult } from './importServers'
const props = defineProps<{ profiles: ServerProfile[]; groups: SshGroup[]; groupId?: string }>()
const emit = defineEmits<{ close: []; changed: [] }>()
const raw = ref(''),
  table = ref<string[][]>([]),
  mapping = ref<Mapping>(mapHeaders([])),
  error = ref('')
const mode = ref<PasswordMode>('local'),
  duplicate = ref<'skip' | 'update' | 'new'>('skip'),
  target = ref(props.groupId ?? 'from-file')
const acknowledged = ref(false),
  busy = ref(false),
  cancelled = ref(false),
  results = ref<ImportResult[]>([]),
  page = ref(0)
let activeRun: Promise<void> | undefined
let disposed = false
onUnmounted(() => {
  disposed = true
  cancelled.value = true
})
const resultByRow = computed(() => new Map(results.value.map((r) => [r.row, r])))
const rows = computed(() => previewRows(table.value, mapping.value, props.profiles))
const visibleRows = computed(() => rows.value.slice(page.value * 100, (page.value + 1) * 100))
const valid = computed(() => rows.value.filter((r) => !r.error).length)
const columns = computed(() => [
  { value: '-1', label: '不导入此列' },
  ...(table.value[0] ?? []).map((label, i) => ({ value: String(i), label })),
])
const groups = computed(() => [
  { value: 'from-file', label: '按文件分组，缺省保留原分组' },
  ...props.groups.map((g) => ({ value: g.id, label: g.name })),
])
useToolLifecycle('ssh', {
  owner: 'ssh.import',
  prepare: () => (busy.value ? '服务器正在导入，请先停止导入' : null),
  dispose: async () => {
    cancelled.value = true
    await activeRun
  },
})
async function choose() {
  try {
    const path = await open({
      multiple: false,
      filters: [{ name: 'CSV / TSV', extensions: ['csv', 'tsv'] }],
    })
    if (!disposed && typeof path === 'string') {
      raw.value = await ipc.sshServerCsvRead(path)
      parse()
    }
  } catch {
    error.value = '文件读取失败，请检查 UTF-8 编码与大小，最大 2 MiB'
  }
}
function parse() {
  try {
    table.value = parseTable(raw.value)
    mapping.value = mapHeaders(table.value[0])
    error.value = ''
    results.value = []
    page.value = 0
  } catch (e) {
    error.value = String(e)
  }
}
async function write(content: string, name: string) {
  try {
    const path = await save({ defaultPath: name, filters: [{ name: 'CSV', extensions: ['csv'] }] })
    if (path) await ipc.sshServerCsvWrite(path, content)
  } catch {
    error.value = '文件保存失败'
  }
}
async function run() {
  if (busy.value || !acknowledged.value) return
  busy.value = true
  cancelled.value = false
  results.value = []
  error.value = ''
  try {
    activeRun = importServers(rows.value, props.profiles, props.groups, {
      passwordMode: mode.value,
      duplicate: duplicate.value,
      groupId: target.value === 'from-file' ? undefined : target.value,
      cancelled: () => cancelled.value,
      onResult: (r) => results.value.push(r),
    })
    await activeRun
  } finally {
    busy.value = false
    emit('changed')
  }
}
function setMode(value: string) {
  if (value === 'local' || value === 'vault' || value === 'none') mode.value = value
}
function setDuplicate(value: string) {
  if (value === 'skip' || value === 'update' || value === 'new') duplicate.value = value
}
function resetSource() {
  table.value = []
  results.value = []
}
function close() {
  if (busy.value) {
    cancelled.value = true
    error.value = '正在停止，将在当前记录保存结束后停止；完成后可关闭。'
    return
  }
  raw.value = ''
  table.value = []
  emit('close')
}
const report = computed(() =>
  csvText([
    ['row', 'status', 'message'],
    ...results.value.map((r) => [String(r.row), r.status, r.message]),
  ])
)
</script>

<template>
  <UiModal open title="批量导入服务器" size="xl" @close="close">
    <div class="flex flex-col gap-md">
      <UiAlert v-if="error" tone="danger">{{ error }}</UiAlert>
      <template v-if="!table.length">
        <UiToolbar
          ><UiButton @click="choose">选择 CSV 文件</UiButton
          ><UiButton variant="ghost" @click="write(serverCsvTemplate, 'servers-template.csv')"
            >下载模板</UiButton
          ></UiToolbar
        >
        <UiTextarea
          v-model="raw"
          :rows="8"
          placeholder="粘贴带表头的 CSV 或 Excel 表格（UTF-8，最多 5000 行）。仅支持用户名与密码认证。"
        />
        <UiButton :disabled="!raw" @click="parse">解析并预览</UiButton>
      </template>
      <template v-else>
        <div class="grid grid-cols-4 gap-sm">
          <UiField v-for="field in fields" :key="field" :label="field"
            ><UiSelect
              :model-value="String(mapping[field])"
              :options="columns"
              :disabled="busy || !!results.length"
              @update:model-value="mapping[field] = Number($event)"
          /></UiField>
        </div>
        <div class="grid grid-cols-3 gap-md">
          <UiField label="目标分组"
            ><UiSelect v-model="target" :options="groups" :disabled="busy || !!results.length"
          /></UiField>
          <UiField label="已存在的连接"
            ><UiSelect
              :model-value="duplicate"
              :options="[
                { value: 'skip', label: '跳过（默认）' },
                { value: 'update', label: '更新唯一匹配项' },
                { value: 'new', label: '仍然新增' },
              ]"
              :disabled="busy || !!results.length"
              @update:model-value="setDuplicate"
          /></UiField>
          <UiField label="密码保存方式"
            ><UiSelect
              :model-value="mode"
              :options="[
                { value: 'local', label: '本地明文保存' },
                { value: 'vault', label: '保存到凭证库' },
                { value: 'none', label: '不保存密码' },
              ]"
              :disabled="busy || !!results.length"
              @update:model-value="setMode"
          /></UiField>
        </div>
        <UiAlert tone="info"
          >本地保存的密码以后可选择导出；保存到凭证库后不会导出密码；不保存时连接需重新输入。更新记录中密码为空时保留原认证。新导入不会自动连接。</UiAlert
        >
        <UiCheckbox
          v-model="acknowledged"
          label="我已了解上述密码保存与导出规则"
          :disabled="busy"
        />
        <UiToolbar :title="'共 ' + rows.length + ' 条 · 校验通过 ' + valid + ' 条'"
          ><template #trailing
            ><UiButton size="sm" variant="ghost" :disabled="page === 0" @click="page--"
              >上一页</UiButton
            ><span class="text-caption">{{ page + 1 }}</span
            ><UiButton
              size="sm"
              variant="ghost"
              :disabled="(page + 1) * 100 >= rows.length"
              @click="page++"
              >下一页</UiButton
            ></template
          ></UiToolbar
        >
        <UiScrollArea class="max-h-[260px]"
          ><UiTable density="compact"
            ><thead>
              <tr>
                <UiTableCell as="th">行</UiTableCell>
                <UiTableCell as="th">名称</UiTableCell>
                <UiTableCell as="th">主机</UiTableCell>
                <UiTableCell as="th">用户名</UiTableCell>
                <UiTableCell as="th">密码</UiTableCell>
                <UiTableCell as="th">校验 / 结果</UiTableCell>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in visibleRows" :key="row.row">
                <UiTableCell>{{ row.row }}</UiTableCell>
                <UiTableCell>{{ row.name }}</UiTableCell>
                <UiTableCell>{{ row.host }}:{{ row.port }}</UiTableCell>
                <UiTableCell>{{ row.username }}</UiTableCell>
                <UiTableCell>{{ row.password ? '已提供' : '空' }}</UiTableCell>
                <UiTableCell class="select-text">
                  {{
                    resultByRow.get(row.row)?.message ??
                    (row.error ||
                      (row.matches.length ? '已存在 ' + row.matches.length + ' 项' : '新增'))
                  }}
                </UiTableCell>
              </tr>
            </tbody></UiTable
          ></UiScrollArea
        >
        <UiAlert v-if="results.length" tone="info"
          >已处理 {{ results.length }} 条，成功
          {{ results.filter((r) => r.status === '成功').length }}，失败
          {{ results.filter((r) => r.status === '失败').length }}，跳过
          {{ results.filter((r) => r.status === '跳过').length }}。</UiAlert
        >
      </template>
    </div>
    <template #footer>
      <UiButton
        v-if="table.length && !results.length"
        :disabled="!acknowledged || !valid || busy"
        @click="run"
        >开始导入</UiButton
      >
      <UiButton v-if="busy" variant="ghost" @click="cancelled = true">完成当前条目后停止</UiButton>
      <UiButton
        v-if="results.length && !busy"
        variant="ghost"
        @click="write(report, 'import-report.csv')"
        >导出脱敏报告</UiButton
      >
      <UiButton v-if="table.length && !busy" variant="ghost" @click="resetSource"
        >重新选择</UiButton
      >
      <UiButton variant="ghost" @click="close">关闭</UiButton>
    </template>
  </UiModal>
</template>
