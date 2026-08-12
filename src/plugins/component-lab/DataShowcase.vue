<script setup lang="ts">
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
  { name: 'production-api / 生产接口', host: '10.0.0.12', status: '在线', latency: '18 ms' },
  { name: 'staging-db / 预发布库', host: '10.0.1.24', status: '在线', latency: '32 ms' },
  { name: 'legacy-worker / 旧任务', host: '10.0.2.08', status: '离线', latency: '—' },
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
</template>
