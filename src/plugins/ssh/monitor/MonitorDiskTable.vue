<script setup lang="ts">
/** 磁盘分区明细表：按使用率降序，>85% 警告色、>95% 危险色（UiDataGrid 承载） */
import { computed } from 'vue'
import type { MonitorData, SshDiskEntry } from '../contracts'
import { formatBytes } from '../connection/useSsh'
import {
  diskUsageTone,
  formatPercent,
  toDiskRows,
  type DiskRow,
  type DiskUsageTone,
} from './sshSystemInfo'
import { UiDataGrid, UiProgress, type UiDataGridColumn } from '@/core/ui'

const props = defineProps<{
  /** 磁盘分区明细（df -hlPT 解析结果） */
  disks: SshDiskEntry[]
  /** 采集中：无数据时显示等待态而不是「无数据」 */
  loading?: boolean
  metrics?: MonitorData | null
}>()

const columns: UiDataGridColumn[] = [
  { key: 'mountPoint', label: '挂载点', width: 110, content: 'technical' },
  { key: 'usePercent', label: '使用率', width: 130, align: 'right' },
  { key: 'usageText', label: '已用 / 总量', width: 130, align: 'right', content: 'numeric' },
  { key: 'fsType', label: '类型', width: 80 },
  { key: 'filesystem', label: '文件系统', width: 150, content: 'technical' },
]

const rows = computed<DiskRow[]>(() => toDiskRows(props.disks))

function progressTone(percent: number) {
  const tone = diskUsageTone(percent)
  return tone === 'normal' ? 'accent' : tone
}

/** 使用率配色（Tailwind 静态类名，避免动态拼接导致样式丢失） */
const USAGE_CLASS: Record<DiskUsageTone, string> = {
  normal: 'text-secondary dark:text-secondary-dark',
  warning: 'text-warning-strong dark:text-warning-dark',
  danger: 'font-semibold text-danger-strong dark:text-danger-dark',
}
</script>

<template>
  <section class="min-w-0" aria-label="磁盘分区">
    <div v-if="metrics" class="mb-[14px]">
      <div class="mb-[8px] flex flex-wrap items-baseline justify-between gap-[6px]">
        <span class="text-body-sm font-medium text-primary dark:text-primary-dark"
          >根分区 <span class="font-mono">/</span></span
        >
        <span class="select-text font-mono text-caption text-secondary dark:text-secondary-dark"
          >{{ formatBytes(metrics.diskUsed) }} / {{ formatBytes(metrics.diskTotal) }} ·
          {{ formatPercent(metrics.diskPercent) }}</span
        >
      </div>
      <UiProgress
        :value="metrics.diskPercent"
        :tone="progressTone(metrics.diskPercent)"
        size="sm"
        aria-label="根分区使用率"
      />
    </div>
    <div class="mb-[8px] flex items-center justify-between gap-[8px] text-caption">
      <h3 class="font-medium text-secondary dark:text-secondary-dark">
        磁盘分区 <span class="font-mono">{{ rows.length }}</span>
      </h3>
      <span class="text-text-muted dark:text-text-muted-dark">按使用率降序</span>
    </div>
    <UiDataGrid
      :columns="columns"
      :rows="rows"
      aria-label="磁盘分区明细"
      :row-numbers="false"
      :height="`${Math.min(7, Math.max(2, rows.length)) * 32 + 36}px`"
      class="rounded-md border border-border dark:border-border-dark"
    >
      <template #cell-usePercent="{ value }">
        <div class="flex items-center gap-[8px]">
          <UiProgress
            class="w-[48px] shrink-0"
            :value="Number(value)"
            :tone="progressTone(Number(value))"
            size="xs"
            aria-label="分区使用率"
          />
          <span class="ml-auto font-mono" :class="USAGE_CLASS[diskUsageTone(Number(value))]">{{
            formatPercent(Number(value))
          }}</span>
        </div>
      </template>
      <template #empty>
        {{ loading ? '正在采集磁盘分区…' : '未采集到磁盘分区数据' }}
      </template>
    </UiDataGrid>
  </section>
</template>
