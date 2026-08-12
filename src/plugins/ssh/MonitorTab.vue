<script setup lang="ts">
/**
 * MonitorTab · 资源监控子页签
 * CPU / 内存 / 磁盘 / 网络 四卡片 + 迷你折线图（3s 轮询后端监控命令）
 */
import { onMounted, onUnmounted, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, MonitorData } from './contracts'
import { formatBytes } from './useSsh'
import { ipc } from './ipc'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
}>()

const data = ref<MonitorData | null>(null)
const errorMessage = ref('')
const refreshing = ref(false)

const history = ref<MonitorData[]>([])
let timer: number | null = null

async function refresh() {
  const connectionId = props.connection?.sessionId
  if (!connectionId || refreshing.value) return
  refreshing.value = true
  try {
    const d = await ipc.sshMonitorGet(connectionId)
    if (props.connection?.sessionId !== connectionId) return
    errorMessage.value = ''
    data.value = d
    history.value.push(d)
    if (history.value.length > 60) history.value.shift()
  } catch (e) {
    if (props.connection?.sessionId !== connectionId) return
    errorMessage.value = `刷新失败：${e}`
  } finally {
    refreshing.value = false
  }
}

onMounted(() => {
  refresh()
  timer = window.setInterval(refresh, 3000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})

watch(
  () => props.connection?.sessionId,
  (sessionId) => {
    data.value = null
    history.value = []
    errorMessage.value = ''
    if (sessionId) void refresh()
  }
)

function sparkline(values: number[], width = 80, height = 24): string {
  if (values.length < 2) return ''
  const max = Math.max(...values, 100)
  const min = 0
  const range = max - min || 1
  const step = width / (values.length - 1)
  const points = values.map((v, i) => `${i * step},${height - ((v - min) / range) * height}`)
  return `M${points.join(' L')}`
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 工具栏 -->
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark"> 资源监控 </span>
      <span class="text-caption text-text-muted dark:text-text-muted-dark"> 每 3s 自动刷新 </span>
      <span v-if="errorMessage" class="text-caption text-danger-strong dark:text-danger-dark">
        {{ errorMessage }}
      </span>
      <div class="ml-auto">
        <button class="btn-ghost !px-[8px] !py-[3px] text-caption" @click="refresh">刷新</button>
      </div>
    </div>

    <!-- 监控卡片 -->
    <div v-if="data" class="grid min-h-0 flex-1 grid-cols-2 gap-[12px] overflow-y-auto p-[12px]">
      <!-- CPU -->
      <div
        class="rounded-lg border border-border bg-surface p-[14px] dark:border-border-dark dark:bg-surface-dark"
      >
        <div class="flex items-center justify-between">
          <span class="text-body-sm text-text-muted dark:text-text-muted-dark">CPU</span>
          <span class="text-display font-mono font-medium text-primary dark:text-primary-dark">
            {{ data.cpuPercent.toFixed(0) }}%
          </span>
        </div>
        <svg class="mt-[8px] h-[40px] w-full" preserveAspectRatio="none">
          <path
            :d="sparkline(history.map((h) => h.cpuPercent))"
            fill="none"
            stroke="var(--color-tertiary)"
            stroke-width="2"
          />
        </svg>
      </div>

      <!-- 内存 -->
      <div
        class="rounded-lg border border-border bg-surface p-[14px] dark:border-border-dark dark:bg-surface-dark"
      >
        <div class="flex items-center justify-between">
          <span class="text-body-sm text-text-muted dark:text-text-muted-dark">内存</span>
          <span class="text-display font-mono font-medium text-primary dark:text-primary-dark">
            {{ data.memoryPercent.toFixed(0) }}%
          </span>
        </div>
        <p class="mt-[2px] font-mono text-caption text-text-muted dark:text-text-muted-dark">
          {{ formatBytes(data.memoryUsed) }} / {{ formatBytes(data.memoryTotal) }}
        </p>
        <svg class="mt-[4px] h-[32px] w-full" preserveAspectRatio="none">
          <path
            :d="sparkline(history.map((h) => h.memoryPercent))"
            fill="none"
            stroke="var(--color-info)"
            stroke-width="2"
          />
        </svg>
      </div>

      <!-- 磁盘 -->
      <div
        class="rounded-lg border border-border bg-surface p-[14px] dark:border-border-dark dark:bg-surface-dark"
      >
        <div class="flex items-center justify-between">
          <span class="text-body-sm text-text-muted dark:text-text-muted-dark">磁盘</span>
          <span class="text-display font-mono font-medium text-primary dark:text-primary-dark">
            {{ data.diskPercent.toFixed(0) }}%
          </span>
        </div>
        <p class="mt-[2px] font-mono text-caption text-text-muted dark:text-text-muted-dark">
          {{ formatBytes(data.diskUsed) }} / {{ formatBytes(data.diskTotal) }}
        </p>
        <svg class="mt-[4px] h-[32px] w-full" preserveAspectRatio="none">
          <path
            :d="sparkline(history.map((h) => h.diskPercent))"
            fill="none"
            stroke="var(--color-warning)"
            stroke-width="2"
          />
        </svg>
      </div>

      <!-- 网络 -->
      <div
        class="rounded-lg border border-border bg-surface p-[14px] dark:border-border-dark dark:bg-surface-dark"
      >
        <div class="flex items-center justify-between">
          <span class="text-body-sm text-text-muted dark:text-text-muted-dark">网络</span>
          <span class="font-mono text-body-sm font-medium text-primary dark:text-primary-dark">
            ↑ {{ formatBytes(data.netUploadBps) }}/s ↓ {{ formatBytes(data.netDownloadBps) }}/s
          </span>
        </div>
        <svg class="mt-[8px] h-[40px] w-full" preserveAspectRatio="none">
          <path
            :d="sparkline(history.map((h) => h.netDownloadBps / 1024 / 1024))"
            fill="none"
            stroke="var(--color-success)"
            stroke-width="2"
          />
        </svg>
      </div>
    </div>

    <!-- 状态栏 -->
    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>最后更新：{{ data ? new Date(data.timestamp).toLocaleTimeString() : '-' }}</span>
      <span class="ml-auto">{{ connection?.status === 'connected' ? '监控中' : '未连接' }}</span>
    </div>
  </div>
</template>
