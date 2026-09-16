<script setup lang="ts">
/**
 * ProcessTab · 进程管理子页签（后端 ps 真实数据）
 */
import { computed, ref, watch } from 'vue'
import type { ProcessDetail, ServerConnection, ServerProfile, ProcessInfo } from '../contracts'
import { formatBytes } from '../connection/useSsh'
import { useUiStore } from '@/stores/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import {
  UiButton,
  UiModal,
  UiSearchInput,
  UiSelect as Select,
  UiTable,
  UiTableCell,
} from '@/core/ui'
import { ipc } from '../ipc'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
}>()

const ui = useUiStore()

const keyword = ref('')
const sortBy = ref<'cpu' | 'memory' | 'pid'>('cpu')
const processes = ref<ProcessInfo[]>([])
const pendingKill = ref<{ pid: number; force: boolean } | null>(null)
/** 详情弹窗：null 表示关闭；detailPid 先于数据置位，用于并发打开时的结果归属 */
const detailPid = ref<number | null>(null)
const detail = ref<ProcessDetail | null>(null)
const detailLoading = ref(false)

const filtered = computed(() => {
  let list = [...processes.value]
  const kw = keyword.value.trim().toLowerCase()
  if (kw) {
    list = list.filter(
      (p) =>
        p.command.toLowerCase().includes(kw) ||
        p.user.toLowerCase().includes(kw) ||
        String(p.pid).includes(kw)
    )
  }
  list.sort((a, b) => {
    if (sortBy.value === 'cpu') return b.cpuPercent - a.cpuPercent
    if (sortBy.value === 'memory') return b.memoryPercent - a.memoryPercent
    return a.pid - b.pid
  })
  return list
})

async function refresh() {
  const connectionId = props.connection?.sessionId
  if (!connectionId) return
  try {
    const result = await ipc.sshProcessList({
      connectionId,
      sortBy: sortBy.value,
      keyword: keyword.value.trim() || undefined,
    })
    if (props.connection?.sessionId === connectionId) processes.value = result
  } catch (e) {
    if (props.connection?.sessionId === connectionId) ui.toast(`进程列表加载失败：${e}`)
  }
}

async function kill(pid: number, force = false) {
  if (!props.connection?.sessionId) return
  try {
    const r = await ipc.sshProcessKill(props.connection.sessionId, pid, force)
    if (r.ok) {
      ui.toast(`${force ? '强制结束' : '结束'}进程 ${pid} 成功`)
      refresh()
    } else {
      ui.toast(`结束进程失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`操作失败：${e}`)
  }
}

function requestKill(pid: number, force = false) {
  pendingKill.value = { pid, force }
}

function confirmKill() {
  const pending = pendingKill.value
  pendingKill.value = null
  if (pending) void kill(pending.pid, pending.force)
}

/** 详情弹窗字段行（远端 ps 列序缺列时自动省略） */
const detailRows = computed(() => {
  const d = detail.value
  if (!d) return []
  return [
    { label: '用户', value: d.user },
    { label: '父进程', value: d.ppid === undefined ? undefined : String(d.ppid) },
    { label: '终端', value: d.tty },
    { label: '启动时间', value: d.started },
    { label: 'CPU 时间', value: d.cpuTime },
  ].filter((row): row is { label: string; value: string } => Boolean(row.value))
})

async function loadDetail(pid: number) {
  const connectionId = props.connection?.sessionId
  if (!connectionId) return
  detailLoading.value = true
  try {
    const result = await ipc.sshProcessDetail(connectionId, pid)
    // 期间切了连接或换了进程则丢弃本次结果
    if (detailPid.value === pid && props.connection?.sessionId === connectionId) {
      detail.value = result
      detailLoading.value = false
    }
  } catch (e) {
    if (detailPid.value === pid) {
      closeDetail()
      ui.toast(`进程详情加载失败：${e}`)
    }
  }
}

function openDetail(pid: number) {
  detailPid.value = pid
  detail.value = null
  detailLoading.value = false
  void loadDetail(pid)
}

function closeDetail() {
  detailPid.value = null
  detail.value = null
  detailLoading.value = false
}

function reloadDetail() {
  if (detailPid.value !== null) void loadDetail(detailPid.value)
}

/** 详情里结束进程：关掉弹窗并复用列表的二次确认流程 */
function killFromDetail() {
  const pid = detailPid.value
  closeDetail()
  if (pid !== null) requestKill(pid)
}

watch(
  () => props.connection?.sessionId,
  (sessionId) => {
    closeDetail()
    processes.value = []
    if (sessionId) void refresh()
  },
  { immediate: true }
)
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark"> 进程管理 </span>
      <UiSearchInput
        v-model="keyword"
        size="sm"
        class="!w-[180px]"
        placeholder="搜索进程/PID/用户"
      />
      <Select
        :model-value="sortBy"
        size="sm"
        class="!w-[100px] shrink-0"
        title="排序方式"
        :options="[
          { value: 'cpu', label: '按 CPU' },
          { value: 'memory', label: '按内存' },
          { value: 'pid', label: '按 PID' },
        ]"
        @update:model-value="sortBy = $event as 'cpu' | 'memory' | 'pid'"
      />
      <div class="ml-auto">
        <UiButton variant="ghost" size="sm" title="刷新进程列表" @click="refresh"> 刷新 </UiButton>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto">
      <UiTable :framed="false" :styled="false" table-class="text-body-sm">
        <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
          <tr
            class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
          >
            <UiTableCell as="th" class="w-[70px] whitespace-nowrap px-[12px] py-[8px]"
              >PID</UiTableCell
            >
            <UiTableCell as="th" class="w-[90px] whitespace-nowrap px-[12px] py-[8px]"
              >用户</UiTableCell
            >
            <UiTableCell as="th" class="w-[80px] whitespace-nowrap px-[12px] py-[8px]"
              >CPU%</UiTableCell
            >
            <UiTableCell as="th" class="w-[80px] whitespace-nowrap px-[12px] py-[8px]"
              >MEM%</UiTableCell
            >
            <UiTableCell as="th" class="w-[100px] whitespace-nowrap px-[12px] py-[8px]"
              >内存</UiTableCell
            >
            <UiTableCell as="th" class="px-[12px] py-[8px]">命令</UiTableCell>
            <UiTableCell as="th" class="w-[110px] whitespace-nowrap px-[12px] py-[8px]"
              >操作</UiTableCell
            >
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="p in filtered"
            :key="p.pid"
            class="border-b border-border/50 transition-colors dark:border-border-dark/50"
          >
            <UiTableCell content="numeric" class="px-[12px] py-[8px]">{{ p.pid }}</UiTableCell>
            <UiTableCell content="technical" class="px-[12px] py-[8px]">{{ p.user }}</UiTableCell>
            <UiTableCell content="numeric" class="px-[12px] py-[8px]">{{
              p.cpuPercent.toFixed(1)
            }}</UiTableCell>
            <UiTableCell content="numeric" class="px-[12px] py-[8px]">
              {{ p.memoryPercent.toFixed(1) }}
            </UiTableCell>
            <UiTableCell content="numeric" class="px-[12px] py-[8px]">
              {{ formatBytes(p.memoryBytes) }}
            </UiTableCell>
            <UiTableCell
              content="code"
              class="max-w-[200px] truncate px-[12px] py-[8px]"
              :title="p.command"
            >
              {{ p.command }}
            </UiTableCell>
            <UiTableCell content="action" class="whitespace-nowrap px-[12px] py-[8px]">
              <div class="flex items-center gap-[4px]">
                <UiButton variant="ghost" size="xs" @click="openDetail(p.pid)"> 详情 </UiButton>
                <UiButton variant="ghost" size="xs" @click="requestKill(p.pid)"> 结束 </UiButton>
                <UiButton
                  variant="ghost"
                  size="xs"
                  class="text-danger-strong dark:text-danger-dark"
                  @click="requestKill(p.pid, true)"
                >
                  强杀
                </UiButton>
              </div>
            </UiTableCell>
          </tr>
        </tbody>
      </UiTable>
    </div>

    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>共 {{ filtered.length }} 个进程</span>
      <span class="ml-auto">{{ connection?.status === 'connected' ? '就绪' : '未连接' }}</span>
    </div>
    <ConfirmDialog
      :open="pendingKill !== null"
      :title="pendingKill?.force ? '强制结束进程' : '结束进程'"
      :message="`${pendingKill?.force ? '强制结束' : '结束'}进程 ${pendingKill?.pid ?? ''}？`"
      :confirm-label="pendingKill?.force ? '强制结束' : '结束'"
      :danger="pendingKill?.force"
      @close="pendingKill = null"
      @confirm="confirmKill"
    />

    <UiModal
      :open="detailPid !== null"
      size="md"
      :title="detailPid === null ? '进程详情' : `进程详情 · PID ${detailPid}`"
      @close="closeDetail"
    >
      <div
        v-if="detailLoading"
        class="py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        正在读取进程信息…
      </div>
      <template v-else>
        <p
          v-if="detail && !detail.found"
          class="text-body-sm text-secondary dark:text-secondary-dark"
        >
          远端已无该进程（可能已退出）。
        </p>
        <dl v-else-if="detail" class="grid grid-cols-[84px_1fr] gap-y-[6px] text-body-sm">
          <template v-for="row in detailRows" :key="row.label">
            <dt class="text-text-muted dark:text-text-muted-dark">{{ row.label }}</dt>
            <dd class="break-all text-primary dark:text-primary-dark">{{ row.value }}</dd>
          </template>
        </dl>
        <div v-if="detail?.command" class="mt-[12px]">
          <div class="mb-[4px] text-caption text-text-muted dark:text-text-muted-dark">
            完整命令行
          </div>
          <div
            class="max-h-[120px] overflow-auto break-all whitespace-pre-wrap rounded-[6px] border border-border bg-surface-muted p-[8px] font-mono text-caption text-primary dark:border-border-dark dark:bg-surface-muted-dark dark:text-primary-dark"
          >
            {{ detail.command }}
          </div>
        </div>
        <div v-if="detail?.raw" class="mt-[12px]">
          <div class="mb-[4px] text-caption text-text-muted dark:text-text-muted-dark">
            ps 原始输出
          </div>
          <pre
            class="max-h-[140px] overflow-auto whitespace-pre-wrap rounded-[6px] border border-border bg-surface-muted p-[8px] font-mono text-caption text-primary dark:border-border-dark dark:bg-surface-muted-dark dark:text-primary-dark"
            >{{ detail.raw }}</pre>
        </div>
      </template>
      <div class="mt-[16px] flex items-center justify-end gap-[8px]">
        <UiButton variant="ghost" size="sm" :loading="detailLoading" @click="reloadDetail">
          刷新
        </UiButton>
        <UiButton
          variant="ghost"
          size="sm"
          class="text-danger-strong dark:text-danger-dark"
          :disabled="detail?.found === false"
          @click="killFromDetail"
        >
          结束进程
        </UiButton>
        <UiButton size="sm" @click="closeDetail"> 关闭 </UiButton>
      </div>
    </UiModal>
  </div>
</template>
