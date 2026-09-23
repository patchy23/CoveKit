<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  UiPopover,
  UiButton,
  UiSelect,
  UiSearchInput,
  UiScrollArea,
  UiAlert,
  UiProgress,
} from '@/core/ui'
import { formatBytes } from '../connection/useSsh'
import type { TransferItem } from './useFileTransfer'
const props = defineProps<{ open: boolean; items: TransferItem[] }>()
defineEmits<{
  'update:open': [value: boolean]
  cancel: [id: string]
  clear: []
  retry: [item: TransferItem]
  locate: [item: TransferItem]
}>()
const filter = ref('all'),
  query = ref('')
const options = [
  { value: 'all', label: '全部' },
  { value: 'running', label: '进行中' },
  { value: 'failed', label: '失败' },
  { value: 'done', label: '已完成' },
]
const rows = computed(() =>
  props.items
    .filter(
      (v) =>
        (v.label + v.localPath + v.remotePath).toLowerCase().includes(query.value.toLowerCase()) &&
        (filter.value === 'all' ||
          (filter.value === 'running' && !v.done) ||
          (filter.value === 'failed' && !!v.error) ||
          (filter.value === 'done' && v.done && !v.error && !v.cancelling))
    )
    .reverse()
)
function status(item: TransferItem) {
  return item.done
    ? item.cancelling
      ? '已取消'
      : item.error
        ? '失败'
        : '已完成'
    : item.cancelling
      ? '正在取消'
      : item.preparing
        ? '正在准备'
        : item.kind === 'upload' || item.kind === 'download'
          ? '传输中'
          : '处理中'
}
</script>
<template>
  <UiPopover
    :open="open"
    label="文件任务"
    width="560px"
    side="top"
    @update:open="$emit('update:open', $event)"
  >
    <template #trigger
      ><UiButton size="sm" variant="ghost"
        >传输 {{ items.filter((t) => !t.done).length || '' }}</UiButton
      ></template
    >
    <div class="flex items-center gap-xs border-b border-border p-sm dark:border-border-dark">
      <UiSearchInput
        v-model="query"
        size="sm"
        class="min-w-0 flex-1"
        placeholder="搜索文件或路径"
      />
      <UiSelect v-model="filter" :options="options" size="sm" />
      <UiButton
        size="xs"
        variant="ghost"
        @click="items.filter((v) => !v.done).forEach((v) => $emit('cancel', v.id))"
        >取消全部</UiButton
      >
      <UiButton size="xs" variant="ghost" @click="$emit('clear')">清除已结束</UiButton>
    </div>
    <UiScrollArea class="max-h-[55vh] min-h-0" axis="vertical">
      <div
        v-if="!rows.length"
        class="p-md text-center text-caption text-secondary dark:text-secondary-dark"
      >
        暂无匹配的传输任务
      </div>
      <div
        v-for="item in rows"
        :key="item.id"
        class="space-y-xs border-b border-border p-sm last:border-0 dark:border-border-dark"
      >
        <div class="flex items-center gap-sm">
          <span class="min-w-0 flex-1 truncate text-body-sm"
            >{{
              {
                upload: '上传',
                download: '下载',
                compress: '压缩',
                extract: '解压',
                preview: '预览',
              }[item.kind]
            }}
            · {{ item.label }}</span
          >
          <span class="shrink-0 text-caption text-secondary dark:text-secondary-dark">{{
            status(item)
          }}</span>
          <UiButton
            v-if="!item.done"
            size="xs"
            variant="ghost"
            :disabled="item.cancelling"
            @click="$emit('cancel', item.id)"
            >取消</UiButton
          >
          <UiButton v-else-if="item.error" size="xs" variant="ghost" @click="$emit('retry', item)"
            >重试</UiButton
          >
          <UiButton size="xs" variant="ghost" @click="$emit('locate', item)">定位</UiButton>
        </div>
        <p
          class="truncate select-text font-mono text-caption text-secondary dark:text-secondary-dark"
        >
          {{ item.localPath ? item.localPath + ' ↔ ' : '' }}{{ item.remotePath }}
        </p>
        <UiProgress
          v-if="!item.done"
          :value="item.total > 0 ? (item.transferred / item.total) * 100 : 0"
          :indeterminate="item.total <= 0"
        />
        <p class="text-caption text-secondary dark:text-secondary-dark">
          {{ formatBytes(item.transferred) }} /
          {{ item.total > 0 ? formatBytes(item.total) : '正在统计'
          }}<span v-if="!item.done && item.speed > 0"> · {{ formatBytes(item.speed) }}/s</span>
        </p>
        <UiAlert v-if="item.error" tone="danger" size="xs">{{ item.error }}</UiAlert>
      </div>
      <slot />
    </UiScrollArea>
  </UiPopover>
</template>
