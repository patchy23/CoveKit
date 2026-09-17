<script setup lang="ts">
/**
 * 远程系统信息面板：30 秒自动采集（可暂停）+ 手动刷新，并承载磁盘分区明细表。
 * 采集失败保留上一次数据并显式报错，不静默；切换服务器先清空再拉新。
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import type { MonitorData, ServerConnection, SshSystemInfoResult } from '../contracts'
import { ipc } from '../ipc'
import { formatLoadAvg, textOrDash } from './sshSystemInfo'
import MonitorDiskTable from './MonitorDiskTable.vue'
import { UiButton } from '@/core/ui'

const props = defineProps<{ connection?: ServerConnection; metrics?: MonitorData | null }>()

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
  return updatedAt.value
    ? `最后更新：${new Date(updatedAt.value).toLocaleTimeString()}`
    : '等待采集'
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
  <section>
    <header class="flex flex-wrap items-center gap-x-[12px] gap-y-[6px] px-[16px] py-[10px]">
      <h2 class="text-body font-medium text-primary dark:text-primary-dark">系统与存储</h2>
      <span class="text-caption text-text-muted dark:text-text-muted-dark"
        >每 30 秒采集 · {{ statusText }}</span
      >
      <div class="ml-auto flex items-center gap-[6px]">
        <UiButton variant="ghost" size="xs" @click="togglePause">{{
          paused ? '继续采集' : '暂停采集'
        }}</UiButton>
        <UiButton variant="ghost" size="xs" :loading="loading" @click="refresh"
          >刷新系统信息</UiButton
        >
      </div>
    </header>
    <p
      v-if="errorMessage"
      role="alert"
      class="mx-[16px] mb-[12px] text-body-sm text-danger-strong dark:text-danger-dark"
    >
      {{ errorMessage }}
    </p>
    <div class="monitor-details grid gap-[20px] px-[16px] pb-[16px]">
      <dl class="grid content-start gap-[12px]">
        <div v-for="field in fields" :key="field.label" class="min-w-0">
          <dt class="text-caption text-text-muted dark:text-text-muted-dark">{{ field.label }}</dt>
          <dd
            class="mt-[2px] select-text break-words font-mono text-body-sm text-primary dark:text-primary-dark"
          >
            {{ field.value }}
          </dd>
        </div>
      </dl>
      <MonitorDiskTable :disks="data?.disks ?? []" :loading="loading" :metrics="metrics" />
    </div>
  </section>
</template>

<style scoped>
.monitor-details {
  grid-template-columns: minmax(0, 1fr);
}
.monitor-details > dl {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
@container ssh-monitor (min-width: 820px) {
  .monitor-details {
    grid-template-columns: 220px minmax(0, 1fr);
  }
  .monitor-details > dl {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
