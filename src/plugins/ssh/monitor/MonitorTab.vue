<script setup lang="ts">
/** SSH 资源监控：实时指标与趋势在上，系统信息与存储明细在下。 */
import { computed, onMounted, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, MonitorData } from '../contracts'
import { throttledInterval, useToolScope } from '@/core/lifecycle'
import { UiButton, UiEmptyState, UiScrollArea, UiSpinner } from '@/core/ui'
import { ipc } from '../ipc'
import MonitorSystemPanel from './MonitorSystemPanel.vue'
import MonitorTrend from './MonitorTrend.vue'
import { TREND_WINDOW_MS } from './monitorTrend'

const props = withDefaults(
  defineProps<{ connection?: ServerConnection; profile?: ServerProfile; active?: boolean }>(),
  { active: true, connection: undefined, profile: undefined }
)
const data = ref<MonitorData | null>(null)
const history = ref<MonitorData[]>([])
const errorMessage = ref('')
const refreshing = ref(false)
const { scope, visibility, onResume } = useToolScope('ssh', 'ssh.monitor')
const POLL_VISIBLE_MS = 3000
const POLL_HIDDEN_MS = 30000
const engaged = computed(
  () =>
    props.active && visibility.value.active && !visibility.value.covered && !visibility.value.hidden
)
let pollTimer: ReturnType<typeof setTimeout> | undefined

function cancelPoll() {
  clearTimeout(pollTimer)
  pollTimer = undefined
}
scope.addDispose(cancelPoll)

function schedulePoll(): void {
  cancelPoll()
  if (scope.disposed) return
  pollTimer = setTimeout(
    () => {
      void poll()
    },
    throttledInterval(POLL_VISIBLE_MS, POLL_HIDDEN_MS, !engaged.value)
  )
}

async function poll() {
  cancelPoll()
  await refresh()
  schedulePoll()
}

async function refresh() {
  const connectionId = props.connection?.sessionId
  if (scope.disposed || !connectionId || refreshing.value) return
  refreshing.value = true
  try {
    const sample = await ipc.sshMonitorGet(connectionId)
    if (scope.disposed || props.connection?.sessionId !== connectionId) return
    data.value = sample
    errorMessage.value = ''
    history.value = [
      ...history.value
        .filter((item) => item.timestamp > sample.timestamp - TREND_WINDOW_MS)
        .slice(-119),
      sample,
    ]
  } catch (error) {
    if (!scope.disposed && props.connection?.sessionId === connectionId)
      errorMessage.value = `采集失败：${error}`
  } finally {
    refreshing.value = false
  }
}

onMounted(() => {
  void poll()
  onResume(() => {
    if (engaged.value) void poll()
  })
})

// 功能页、连接页签与工具可见性共同决定频率；返回时立即补样，且始终只有一个待执行计时器。
watch(engaged, (active) => {
  if (active) void poll()
  else schedulePoll()
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
  <div class="monitor-page flex h-full min-h-0 flex-col bg-surface dark:bg-surface-dark">
    <header
      class="flex shrink-0 flex-wrap items-center gap-x-[16px] gap-y-[6px] border-b border-border px-[16px] py-[10px] dark:border-border-dark"
    >
      <h2 class="text-body font-medium text-primary dark:text-primary-dark">资源监控</h2>
      <span class="text-caption text-secondary dark:text-secondary-dark"
        >每 3 秒采样 · 最近 3 分钟</span
      >
      <div class="ml-auto flex items-center gap-[10px]">
        <span class="text-caption text-text-muted dark:text-text-muted-dark">{{
          data ? `更新于 ${new Date(data.timestamp).toLocaleTimeString()}` : '等待采样'
        }}</span>
        <UiButton
          variant="ghost"
          size="sm"
          :loading="refreshing"
          :disabled="!connection"
          @click="refresh"
          >刷新指标</UiButton
        >
      </div>
    </header>
    <UiEmptyState
      v-if="!connection"
      class="m-auto"
      title="尚未建立 SSH 连接"
      description="连接服务器后即可查看资源监控。"
    />
    <UiScrollArea v-else as-child axis="vertical">
      <div class="min-h-0 flex-1">
        <div
          v-if="errorMessage"
          role="alert"
          class="border-b border-warning/30 bg-warning-soft px-[16px] py-[10px] text-body-sm text-warning-strong dark:border-warning-dark/30 dark:bg-warning-soft-dark dark:text-warning-dark"
        >
          {{ errorMessage }}<span v-if="data"> · 当前保留上次采样数据</span>
        </div>
        <div
          v-if="data"
          class="monitor-metrics grid gap-px border-b border-border bg-border dark:border-border-dark dark:bg-border-dark"
        >
          <MonitorTrend
            v-for="kind in ['cpu', 'memory', 'network'] as const"
            :key="kind"
            :kind="kind"
            :data="data"
            :history="history"
          />
        </div>
        <div
          v-else-if="refreshing"
          role="status"
          class="flex min-h-[240px] items-center justify-center gap-[8px] text-body-sm text-secondary dark:text-secondary-dark"
        >
          <UiSpinner />正在采集资源数据…
        </div>
        <UiEmptyState
          v-else
          title="暂未获取资源数据"
          description="可点击刷新指标重试，系统信息与磁盘仍会独立采集。"
        />
        <MonitorSystemPanel :connection="connection" :metrics="data" :active="engaged" />
      </div>
    </UiScrollArea>
  </div>
</template>

<style scoped>
.monitor-page {
  container: ssh-monitor / inline-size;
}
.monitor-metrics {
  grid-template-columns: minmax(0, 1fr);
}
@container ssh-monitor (min-width: 580px) {
  .monitor-metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .monitor-metrics > :last-child {
    grid-column: 1 / -1;
  }
}
@container ssh-monitor (min-width: 900px) {
  .monitor-metrics {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  .monitor-metrics > :last-child {
    grid-column: auto;
  }
}
</style>
