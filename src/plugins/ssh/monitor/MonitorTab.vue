<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/** SSH 资源监控：关键指标概览、容量进度和 3 秒趋势。 */
import { computed, onMounted, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, MonitorData } from '../contracts'
import { throttledInterval, useToolScope } from '@/core/lifecycle'
import { formatBytes } from '../connection/useSsh'
import { ipc } from '../ipc'
import MonitorSystemPanel from './MonitorSystemPanel.vue'
import { UiButton, UiEmptyState, UiPanel, UiProgress } from '@/core/ui'

const props = defineProps<{ connection?: ServerConnection; profile?: ServerProfile }>()
const data = ref<MonitorData | null>(null)
const history = ref<MonitorData[]>([])
const errorMessage = ref('')
const refreshing = ref(false)

// 采样轮询走作用域：页签切走/设置页覆盖/窗口隐藏时降到 30s（T10-7），
// 回到可见立刻补一次采样；卸载时 scope 统一释放，不会留下关不掉的定时器
const { scope, visibility, onResume } = useToolScope('ssh', 'ssh.monitor')
/** 采样间隔：可见 3s（趋势需要密度），不可见 30s（只为不丢状态） */
const POLL_VISIBLE_MS = 3000
const POLL_HIDDEN_MS = 30000

/** 当前是否处于用户可见状态 */
function engaged(): boolean {
  return visibility.value.active && !visibility.value.covered && !visibility.value.hidden
}

/** 自调度轮询：每轮按当前可见性重新计算间隔 */
function schedulePoll(): void {
  const delay = throttledInterval(POLL_VISIBLE_MS, POLL_HIDDEN_MS, !engaged())
  scope.timeout(() => {
    void refresh().finally(schedulePoll)
  }, delay)
}

async function refresh() {
  const connectionId = props.connection?.sessionId
  if (!connectionId || refreshing.value) return
  refreshing.value = true
  try {
    const sample = await ipc.sshMonitorGet(connectionId)
    if (props.connection?.sessionId !== connectionId) return
    data.value = sample
    errorMessage.value = ''
    history.value = [...history.value.slice(-59), sample]
  } catch (error) {
    if (props.connection?.sessionId === connectionId) errorMessage.value = `刷新失败：${error}`
  } finally {
    refreshing.value = false
  }
}

function trendPath(values: number[], width = 600, height = 120, fixedMax?: number) {
  if (values.length < 2) return ''
  const max = fixedMax ?? Math.max(...values, 1)
  return values
    .map((value, index) => {
      const x = (index / (values.length - 1)) * width
      const y = height - (Math.min(value, max) / max) * height
      return `${index ? 'L' : 'M'}${x.toFixed(1)},${y.toFixed(1)}`
    })
    .join(' ')
}

const cpuPath = computed(() =>
  trendPath(
    history.value.map((item) => item.cpuPercent),
    600,
    120,
    100
  )
)
const memoryPath = computed(() =>
  trendPath(
    history.value.map((item) => item.memoryPercent),
    600,
    120,
    100
  )
)

onMounted(() => {
  void refresh()
  schedulePoll()
  onResume(() => {
    void refresh()
  })
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
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-surface-muted/40 dark:bg-surface-muted-dark/40">
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border bg-surface px-[14px] py-[8px] dark:border-border-dark dark:bg-surface-dark"
    >
      <div>
        <div class="text-body font-medium text-primary dark:text-primary-dark">服务器资源概览</div>
        <div class="text-caption text-text-muted dark:text-text-muted-dark">
          每 3 秒采样，保留最近 3 分钟趋势
        </div>
      </div>
      <span
        v-if="errorMessage"
        class="ml-[8px] text-caption text-danger-strong dark:text-danger-dark"
      >
        {{ errorMessage }}
      </span>
      <UiButton class="ml-auto" variant="ghost" size="sm" :loading="refreshing" @click="refresh">
        立即刷新
      </UiButton>
    </div>

    <UiEmptyState
      v-if="!connection"
      class="m-auto"
      title="尚未建立 SSH 连接"
      description="双击左侧服务器打开一个终端后，即可查看监控数据。"
    />

    <UiScrollArea v-else-if="data" as-child axis="vertical">
      <div class="min-h-0 flex-1 space-y-[12px] p-[14px]">
        <div class="grid grid-cols-4 gap-[10px]">
          <UiPanel padding="sm">
            <div class="text-caption text-text-muted dark:text-text-muted-dark">CPU 使用率</div>
            <div
              class="mt-[6px] font-mono text-display font-medium text-primary dark:text-primary-dark"
            >
              {{ data.cpuPercent.toFixed(1) }}%
            </div>
            <UiProgress class="mt-[10px]" :value="data.cpuPercent" size="sm" />
          </UiPanel>
          <UiPanel padding="sm">
            <div class="text-caption text-text-muted dark:text-text-muted-dark">内存使用率</div>
            <div
              class="mt-[6px] font-mono text-display font-medium text-primary dark:text-primary-dark"
            >
              {{ data.memoryPercent.toFixed(1) }}%
            </div>
            <div class="mt-[6px] font-mono text-caption text-text-muted dark:text-text-muted-dark">
              {{ formatBytes(data.memoryUsed) }} / {{ formatBytes(data.memoryTotal) }}
            </div>
          </UiPanel>
          <UiPanel padding="sm">
            <div class="text-caption text-text-muted dark:text-text-muted-dark">磁盘使用率</div>
            <div
              class="mt-[6px] font-mono text-display font-medium text-primary dark:text-primary-dark"
            >
              {{ data.diskPercent.toFixed(1) }}%
            </div>
            <div class="mt-[6px] font-mono text-caption text-text-muted dark:text-text-muted-dark">
              {{ formatBytes(data.diskUsed) }} / {{ formatBytes(data.diskTotal) }}
            </div>
          </UiPanel>
          <UiPanel padding="sm">
            <div class="text-caption text-text-muted dark:text-text-muted-dark">实时网络</div>
            <div
              class="mt-[7px] font-mono text-body font-medium text-success-strong dark:text-success-dark"
            >
              ↓ {{ formatBytes(data.netDownloadBps) }}/s
            </div>
            <div
              class="mt-[6px] font-mono text-body font-medium text-info-strong dark:text-info-dark"
            >
              ↑ {{ formatBytes(data.netUploadBps) }}/s
            </div>
          </UiPanel>
        </div>

        <UiPanel
          title="CPU 与内存趋势"
          description="橙色为 CPU，蓝色为内存；纵轴范围 0–100%"
          padding="sm"
        >
          <div
            class="relative h-[180px] overflow-hidden rounded-md bg-surface-muted dark:bg-surface-muted-dark"
          >
            <div
              class="absolute inset-x-0 top-1/4 border-t border-border/60 dark:border-border-dark/60"
            />
            <div
              class="absolute inset-x-0 top-1/2 border-t border-border/60 dark:border-border-dark/60"
            />
            <div
              class="absolute inset-x-0 top-3/4 border-t border-border/60 dark:border-border-dark/60"
            />
            <svg
              viewBox="0 0 600 120"
              preserveAspectRatio="none"
              class="absolute inset-[12px] h-[156px] w-[calc(100%-24px)]"
            >
              <path
                :d="cpuPath"
                fill="none"
                stroke="var(--color-tertiary)"
                stroke-width="2.5"
                vector-effect="non-scaling-stroke"
              />
              <path
                :d="memoryPath"
                fill="none"
                stroke="var(--color-info)"
                stroke-width="2.5"
                vector-effect="non-scaling-stroke"
              />
            </svg>
          </div>
        </UiPanel>

        <div class="grid grid-cols-2 gap-[10px]">
          <UiPanel title="内存容量" padding="sm">
            <UiProgress :value="data.memoryPercent" tone="accent" show-value />
          </UiPanel>
          <UiPanel title="根分区容量" padding="sm">
            <UiProgress :value="data.diskPercent" tone="warning" show-value />
          </UiPanel>
        </div>

        <!-- 系统信息与磁盘明细：采样间隔 30 秒（变化慢，避免与 3 秒指标叠加压力） -->
        <MonitorSystemPanel :connection="connection" />
      </div>
    </UiScrollArea>

    <div
      class="flex shrink-0 border-t border-border bg-surface px-[14px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:bg-surface-dark dark:text-text-muted-dark"
    >
      <span>最后更新：{{ data ? new Date(data.timestamp).toLocaleTimeString() : '-' }}</span>
      <span class="ml-auto">{{ data ? `已采集 ${history.length} 个样本` : '等待数据' }}</span>
    </div>
  </div>
</template>
