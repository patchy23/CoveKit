<script setup lang="ts">
/** 指标当前值与时间趋势放在同一区域；网络双线共享纵轴，虚线表示上传。 */
import { computed } from 'vue'
import type { MonitorData } from '../contracts'
import { formatBytes } from '../connection/useSsh'
import { monitorTrendPath } from './monitorTrend'

const props = defineProps<{
  kind: 'cpu' | 'memory' | 'network'
  data: MonitorData
  history: MonitorData[]
}>()
const isNetwork = computed(() => props.kind === 'network')
const title = computed(() => ({ cpu: 'CPU', memory: '内存', network: '网络' })[props.kind])
const primaryValue = (sample: MonitorData) =>
  props.kind === 'cpu'
    ? sample.cpuPercent
    : props.kind === 'memory'
      ? sample.memoryPercent
      : sample.netDownloadBps
const value = computed(() =>
  isNetwork.value
    ? `${formatBytes(props.data.netDownloadBps)}/s`
    : `${primaryValue(props.data).toFixed(1)}%`
)
const detail = computed(() =>
  props.kind === 'cpu'
    ? '总使用率'
    : props.kind === 'memory'
      ? `${formatBytes(props.data.memoryUsed)} / ${formatBytes(props.data.memoryTotal)}`
      : `上传 ${formatBytes(props.data.netUploadBps)}/s`
)
const maximum = computed(() =>
  isNetwork.value
    ? Math.max(
        1024,
        ...props.history.flatMap((sample) => [sample.netDownloadBps, sample.netUploadBps])
      )
    : 100
)
const paths = computed(() => {
  const primary = monitorTrendPath(
    props.history.map((sample) => ({ timestamp: sample.timestamp, value: primaryValue(sample) })),
    props.data.timestamp,
    maximum.value
  )
  const secondary = isNetwork.value
    ? monitorTrendPath(
        props.history.map((sample) => ({
          timestamp: sample.timestamp,
          value: sample.netUploadBps,
        })),
        props.data.timestamp,
        maximum.value
      )
    : ''
  return { primary, secondary }
})
const axisMax = computed(() => (isNetwork.value ? `${formatBytes(maximum.value)}/s` : '100%'))
</script>

<template>
  <section class="min-w-0 bg-surface p-[16px] dark:bg-surface-dark" :aria-label="`${title}监控`">
    <div class="flex items-center justify-between gap-[8px]">
      <h3 class="text-body font-medium text-secondary dark:text-secondary-dark">{{ title }}</h3>
      <span class="text-caption text-text-muted dark:text-text-muted-dark">{{
        isNetwork ? '下载速率' : '使用率'
      }}</span>
    </div>
    <div
      class="mt-[8px] select-text font-mono text-display font-medium tabular-nums text-primary dark:text-primary-dark"
    >
      {{ value }}
    </div>
    <p
      class="mt-[2px] min-h-[20px] select-text font-mono text-caption text-secondary dark:text-secondary-dark"
    >
      {{ detail }}
    </p>
    <div
      class="mt-[16px] flex items-center justify-between text-caption text-text-muted dark:text-text-muted-dark"
    >
      <span>{{ axisMax }}</span>
      <div v-if="isNetwork" class="flex gap-[10px]">
        <span class="flex items-center gap-[4px]"
          ><i
            class="w-[12px] border-t-2 border-tertiary-strong dark:border-tertiary-dark"
          />下载</span
        >
        <span class="flex items-center gap-[4px]"
          ><i
            class="w-[12px] border-t-2 border-dashed border-secondary dark:border-secondary-dark"
          />上传</span
        >
      </div>
    </div>
    <div class="relative mt-[4px] h-[100px] border-y border-border dark:border-border-dark">
      <div
        class="absolute inset-x-0 top-1/2 border-t border-dashed border-border dark:border-border-dark"
      />
      <svg
        viewBox="0 -3 600 106"
        preserveAspectRatio="none"
        class="relative h-full w-full"
        role="img"
        :aria-label="`${title}最近三分钟趋势，截至 ${new Date(data.timestamp).toLocaleTimeString()}`"
      >
        <path
          :d="paths.primary"
          fill="none"
          class="stroke-tertiary-strong dark:stroke-tertiary-dark"
          stroke-width="2"
          vector-effect="non-scaling-stroke"
        />
        <path
          v-if="isNetwork"
          :d="paths.secondary"
          fill="none"
          class="stroke-secondary dark:stroke-secondary-dark"
          stroke-width="2"
          stroke-dasharray="4 3"
          vector-effect="non-scaling-stroke"
        />
      </svg>
      <span
        v-if="history.length < 2"
        class="absolute inset-0 flex items-center justify-center text-caption text-text-muted dark:text-text-muted-dark"
        >等待下一次采样</span
      >
    </div>
    <div
      class="mt-[4px] flex justify-between text-caption text-text-muted dark:text-text-muted-dark"
    >
      <span>3 分钟前</span><span>本次采样</span>
    </div>
  </section>
</template>
