<script setup lang="ts">
/**
 * 远程系统信息面板：30 秒自动采集（可暂停）+ 手动刷新，并承载磁盘分区明细表。
 * 采集失败保留上一次数据并显式报错，不静默；切换服务器先清空再拉新。
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import type { ServerConnection, SshSystemInfoResult } from './contracts'
import { ipc } from './ipc'
import { formatLoadAvg, textOrDash } from './sshSystemInfo'
import MonitorDiskTable from './MonitorDiskTable.vue'
import { UiButton, UiPanel } from '@/core/ui'

const props = defineProps<{ connection?: ServerConnection }>()

/** 自动刷新间隔：系统信息变化慢，30 秒足够，避免与 3 秒指标轮询叠加压力 */
const REFRESH_MS = 30000

const data = ref<SshSystemInfoResult | null>(null)
const loading = ref(false)
const paused = ref(false)
const errorMessage = ref('')
const updatedAt = ref(0)
let timer: number | null = null

/** 采集一次（并发去重；连接已切换时丢弃迟到响应，避免串台） */
async function refresh() {
  const connectionId = props.connection?.sessionId
  if (!connectionId || loading.value) return
  loading.value = true
  try {
    const result = await ipc.sshSystemInfoGet(connectionId)
    if (props.connection?.sessionId !== connectionId) return
    data.value = result
    updatedAt.value = Date.now()
    errorMessage.value = ''
  } catch (error) {
    if (props.connection?.sessionId === connectionId) {
      errorMessage.value = `系统信息采集失败：${error}`
    }
  } finally {
    loading.value = false
  }
}

/** 暂停/恢复自动采集（暂停后仍可手动刷新） */
function togglePause() {
  paused.value = !paused.value
}

const info = computed(() => data.value?.info ?? null)
/** uptime 与负载来自同一条命令，该段缺失时一起显示 `-`，避免把 0.00 误读为「空闲」 */
const uptimeAvailable = computed(() => Boolean(info.value?.uptimeText))
const fields = computed<{ label: string; value: string }[]>(() => [
  { label: '主机名', value: textOrDash(info.value?.hostname) },
  { label: '发行版', value: textOrDash(info.value?.osName) },
  { label: '内核', value: textOrDash(info.value?.kernel) },
  { label: '运行时长', value: uptimeAvailable.value ? textOrDash(info.value?.uptimeText) : '-' },
  {
    label: '平均负载 1/5/15',
    value: uptimeAvailable.value && info.value ? formatLoadAvg(info.value.loadAvg) : '-',
  },
  {
    label: '逻辑核数',
    value: info.value && info.value.cpuCores > 0 ? String(info.value.cpuCores) : '-',
  },
])
const statusText = computed(() => {
  if (loading.value) return '采集中…'
  if (paused.value) return '已暂停自动刷新'
  return updatedAt.value ? `最后更新：${new Date(updatedAt.value).toLocaleTimeString()}` : '等待采集'
})

/** 供容器「立即刷新」复用（指标与系统信息同一次刷新） */
defineExpose({ refresh })

onMounted(() => {
  void refresh()
  timer = window.setInterval(() => {
    if (!paused.value) void refresh()
  }, REFRESH_MS)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})

watch(
  () => props.connection?.sessionId,
  (sessionId) => {
    data.value = null
    errorMessage.value = ''
    updatedAt.value = 0
    if (sessionId) void refresh()
  }
)
</script>

<template>
  <UiPanel title="远程系统信息" description="每 30 秒自动采集，可暂停或手动刷新" padding="sm">
    <template #actions>
      <div class="flex items-center gap-[8px]">
        <UiButton variant="ghost" size="sm" @click="togglePause">
          {{ paused ? '继续自动刷新' : '暂停自动刷新' }}
        </UiButton>
        <UiButton variant="ghost" size="sm" :loading="loading" @click="refresh">
          立即刷新
        </UiButton>
      </div>
    </template>

    <dl class="grid grid-cols-2 gap-x-[16px] gap-y-[10px] md:grid-cols-3">
      <div v-for="field in fields" :key="field.label" class="min-w-0">
        <dt class="text-caption text-text-muted dark:text-text-muted-dark">{{ field.label }}</dt>
        <dd
          class="mt-[2px] truncate font-mono text-body-sm text-secondary dark:text-secondary-dark"
          :title="field.value"
        >
          {{ field.value }}
        </dd>
      </div>
    </dl>

    <p
      v-if="errorMessage"
      class="mt-[10px] rounded-md border border-danger/40 px-[8px] py-[6px] text-caption text-danger-strong dark:border-danger-dark/40 dark:text-danger-dark"
    >
      {{ errorMessage }}
    </p>

    <div class="mt-[10px] flex items-center text-caption text-text-muted dark:text-text-muted-dark">
      <span>{{ statusText }}</span>
      <span class="ml-auto">数据来源：hostname / os-release / uname / uptime / nproc / df -hlPT</span>
    </div>

    <MonitorDiskTable class="mt-[12px]" :disks="data?.disks ?? []" :loading="loading" />
  </UiPanel>
</template>
