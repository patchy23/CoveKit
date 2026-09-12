<script setup lang="ts">
/** 磁盘分区明细表：按使用率降序，>85% 警告色、>95% 危险色（UiDataGrid 承载） */
import { computed } from 'vue'
import type { SshDiskEntry } from '../contracts'
import {
  diskUsageTone,
  formatPercent,
  toDiskRows,
  type DiskRow,
  type DiskUsageTone,
} from './sshSystemInfo'
import { UiDataGrid, UiPanel, type UiDataGridColumn } from '@/core/ui'

const props = defineProps<{
  /** 磁盘分区明细（df -hlPT 解析结果） */
  disks: SshDiskEntry[]
  /** 采集中：无数据时显示等待态而不是「无数据」 */
  loading?: boolean
}>()

const columns: UiDataGridColumn[] = [
  { key: 'mountPoint', label: '挂载点', width: 200 },
  { key: 'fsType', label: '类型', width: 96 },
  { key: 'filesystem', label: '文件系统', width: 200 },
  { key: 'usageText', label: '已用 / 总量', width: 130, align: 'right' },
  { key: 'usePercent', label: '使用率', width: 90, align: 'right' },
]

const rows = computed<DiskRow[]>(() => toDiskRows(props.disks))

/** 使用率配色（Tailwind 静态类名，避免动态拼接导致样式丢失） */
const USAGE_CLASS: Record<DiskUsageTone, string> = {
  normal: 'text-secondary dark:text-secondary-dark',
  warning: 'text-warning-strong dark:text-warning-dark',
  danger: 'font-semibold text-danger-strong dark:text-danger-dark',
}
</script>

<template>
  <UiPanel
    title="磁盘分区"
    :description="`共 ${rows.length} 个分区，按使用率降序；超过 85% 标黄、95% 标红`"
    padding="sm"
  >
    <UiDataGrid
      :columns="columns"
      :rows="rows"
      aria-label="磁盘分区明细"
      height="240px"
      class="rounded-md border border-border dark:border-border-dark"
    >
      <template #cell-usePercent="{ value }">
        <span :class="USAGE_CLASS[diskUsageTone(Number(value))]">
          {{ formatPercent(Number(value)) }}
        </span>
      </template>
      <template #empty>
        {{ loading ? '正在采集磁盘分区…' : '未采集到磁盘分区数据（df -hlPT 无输出）' }}
      </template>
    </UiDataGrid>
  </UiPanel>
</template>
