<script setup lang="ts">
/**
 * ProcessTab · 进程管理子页签（后端 ps 真实数据）
 */
import { computed, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, ProcessInfo } from './contracts'
import { formatBytes } from './useSsh'
import { useUiStore } from '@/stores/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { UiButton, UiSearchInput, UiSelect as Select } from '@/core/ui'
import { ipc } from './ipc'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
}>()

const ui = useUiStore()

const keyword = ref('')
const sortBy = ref<'cpu' | 'memory' | 'pid'>('cpu')
const processes = ref<ProcessInfo[]>([])
const pendingKill = ref<{ pid: number; force: boolean } | null>(null)

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

watch(
  () => props.connection?.sessionId,
  (sessionId) => {
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
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? '未连接' }} · 进程管理
      </span>
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
      <table class="data-table">
        <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
          <tr
            class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
          >
            <th class="w-[70px] whitespace-nowrap px-[12px] py-[8px] font-medium">PID</th>
            <th class="w-[90px] whitespace-nowrap px-[12px] py-[8px] font-medium">用户</th>
            <th class="w-[80px] whitespace-nowrap px-[12px] py-[8px] font-medium">CPU%</th>
            <th class="w-[80px] whitespace-nowrap px-[12px] py-[8px] font-medium">MEM%</th>
            <th class="w-[100px] whitespace-nowrap px-[12px] py-[8px] font-medium">内存</th>
            <th class="px-[12px] py-[8px] font-medium">命令</th>
            <th class="w-[110px] whitespace-nowrap px-[12px] py-[8px] font-medium">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="p in filtered"
            :key="p.pid"
            class="border-b border-border/50 transition-colors hover:bg-surface-muted dark:border-border-dark/50 dark:hover:bg-surface-muted-dark"
          >
            <td class="data-cell-tech px-[12px] py-[8px]">{{ p.pid }}</td>
            <td class="data-cell-tech px-[12px] py-[8px]">{{ p.user }}</td>
            <td class="data-cell-tech px-[12px] py-[8px]">{{ p.cpuPercent.toFixed(1) }}</td>
            <td class="data-cell-tech px-[12px] py-[8px]">
              {{ p.memoryPercent.toFixed(1) }}
            </td>
            <td class="data-cell-tech px-[12px] py-[8px]">
              {{ formatBytes(p.memoryBytes) }}
            </td>
            <td class="data-cell-tech max-w-[200px] truncate px-[12px] py-[8px]" :title="p.command">
              {{ p.command }}
            </td>
            <td class="data-cell-action whitespace-nowrap px-[12px] py-[8px]">
              <div class="flex items-center gap-[4px]">
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
            </td>
          </tr>
        </tbody>
      </table>
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
  </div>
</template>
