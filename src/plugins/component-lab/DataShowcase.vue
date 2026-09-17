<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
import { UiScrollArea } from '@/core/ui'
import { ref } from 'vue'
import AppIcon from '@/features/ui/AppIcon.vue'
import {
  UiBadge,
  UiButton,
  UiEmptyState,
  UiPanel,
  UiProgress,
  UiSelect,
  UiSkeleton,
  UiTable,
  UiTableCell,
} from '@/core/ui'

const density = ref<'compact' | 'default' | 'comfortable'>('compact')
const densityOptions = [
  { value: 'compact', label: '紧凑' },
  { value: 'default', label: '标准' },
  { value: 'comfortable', label: '舒适' },
]
const rows = [
  {
    name: 'production-api / 生产接口',
    host: '10.0.0.12',
    command: '/usr/local/bin/api --port 8080',
    status: '在线',
    latency: '18 ms',
  },
  {
    name: 'staging-db / 预发布库',
    host: '10.0.1.24',
    command: 'postgres -D /var/lib/postgresql',
    status: '在线',
    latency: '32 ms',
  },
  {
    name: 'legacy-worker / 旧任务',
    host: '10.0.2.08',
    command: 'python worker.py --legacy',
    status: '离线',
    latency: '—',
  },
]
</script>

<template>
  <UiPanel
    title="数据表格"
    description="表格密度独立于页面尺寸，SSH、数据库和 DNS 列表默认使用 compact。"
  >
    <template #actions>
      <UiSelect
        :model-value="density"
        size="sm"
        class="w-[100px]"
        :options="densityOptions"
        @update:model-value="density = $event as typeof density"
      />
    </template>
    <UiTable :density="density" striped>
      <thead>
        <tr>
          <UiTableCell as="th">服务器</UiTableCell>
          <UiTableCell as="th">地址</UiTableCell>
          <UiTableCell as="th">命令</UiTableCell>
          <UiTableCell as="th">状态</UiTableCell>
          <UiTableCell as="th">延迟</UiTableCell>
          <UiTableCell as="th">操作</UiTableCell>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows" :key="row.name">
          <UiTableCell content="technical" class="text-primary dark:text-primary-dark">
            {{ row.name }}
          </UiTableCell>
          <UiTableCell content="technical">{{ row.host }}</UiTableCell>
          <UiTooltip :content="row.command">
            <UiTableCell content="code" class="max-w-[220px] truncate">
              {{ row.command }}
            </UiTableCell>
          </UiTooltip>
          <UiTableCell content="status">
            <UiBadge :tone="row.status === '在线' ? 'success' : 'danger'" size="xs">{{
              row.status
            }}</UiBadge>
          </UiTableCell>
          <UiTableCell content="numeric">{{ row.latency }}</UiTableCell>
          <UiTableCell content="action"
            ><UiButton variant="ghost" size="xs">详情</UiButton></UiTableCell
          >
        </tr>
      </tbody>
    </UiTable>
  </UiPanel>

  <div class="grid gap-md lg:grid-cols-2">
    <UiPanel title="进度" description="任务进度、资源占用和传输状态。">
      <div class="flex flex-col gap-md">
        <UiProgress :value="24" size="xs" label="xs" show-value />
        <UiProgress :value="48" size="sm" tone="accent" label="sm" show-value />
        <UiProgress :value="72" size="md" tone="warning" label="md" show-value />
        <UiProgress :value="91" size="lg" tone="success" label="lg" show-value />
      </div>
    </UiPanel>

    <UiPanel title="骨架屏" description="列表和详情首次加载时保持布局稳定。">
      <div class="flex gap-md">
        <UiSkeleton variant="circle" width="42px" />
        <div class="min-w-0 flex-1"><UiSkeleton :lines="3" /></div>
      </div>
      <UiSkeleton class="mt-md" variant="rect" height="72px" />
    </UiPanel>
  </div>

  <UiPanel title="空状态" padding="none">
    <UiEmptyState title="还没有查询结果" description="输入查询条件并执行后，结果会显示在这里。">
      <template #icon><AppIcon name="search" :size="22" /></template>
      <UiButton variant="primary" size="sm">开始查询</UiButton>
    </UiEmptyState>
  </UiPanel>

  <UiPanel title="公共滚动区" description="横向、纵向与双向使用同一套滚动条，支持固定暗色主题。">
    <div class="grid min-w-0 gap-md lg:grid-cols-3">
      <div class="min-w-0">
        <p class="mb-sm text-body-sm">纵向</p>
        <UiScrollArea
          axis="vertical"
          class="h-[140px] rounded-md border border-border p-sm dark:border-border-dark"
        >
          <p v-for="row in 12" :key="row" class="py-xs text-body-sm">列表条目 {{ row }}</p>
        </UiScrollArea>
      </div>
      <div class="min-w-0">
        <p class="mb-sm text-body-sm">横向</p>
        <UiScrollArea
          axis="horizontal"
          class="rounded-md border border-border p-sm dark:border-border-dark"
        >
          <div class="flex w-max gap-sm">
            <UiBadge v-for="item in 12" :key="item" tone="neutral">项目 {{ item }}</UiBadge>
          </div>
        </UiScrollArea>
      </div>
      <div class="min-w-0">
        <p class="mb-sm text-body-sm">双向 · 固定暗色</p>
        <UiScrollArea
          axis="both"
          theme="dark"
          class="h-[140px] rounded-md bg-surface-dark p-sm text-secondary-dark"
        >
          <div class="w-[600px] font-mono text-body-sm">
            <p v-for="row in 12" :key="row" class="py-xs">
              {{ row }} · 长内容用于检查横纵滚动条及交汇角落
            </p>
          </div>
        </UiScrollArea>
      </div>
    </div>
  </UiPanel>
</template>
