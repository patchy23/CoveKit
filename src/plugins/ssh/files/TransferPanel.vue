<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  UiPopover,
  UiButton,
  UiSelect,
  UiSearchInput,
  UiScrollArea,
  UiIconButton,
  UiIcon,
  UiTooltip,
  UiModal,
  UiProgress,
} from '@/core/ui'
import { formatBytes } from '../connection/useSsh'
import type { TransferItem } from './useFileTransfer'
const props = defineProps<{ open: boolean; items: TransferItem[] }>()
defineEmits<{
  'update:open': [value: boolean]
  cancel: [id: string]
  'cancel-all': []
  clear: []
  retry: [item: TransferItem]
  locate: [item: TransferItem]
}>()
const detail = ref<TransferItem>()
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
          (filter.value === 'failed' && !!v.error && !v.cancelling) ||
          (filter.value === 'done' && v.done && !v.error && !v.cancelling))
    )
    .reverse()
)
function status(item: TransferItem) {
  return item.done
    ? item.cancelling
      ? '已取消'
      : item.error
        ? { download: '下载', upload: '上传', compress: '压缩', extract: '解压', preview: '预览' }[
            item.kind
          ] +
          '失败：' +
          item.error
        : '已完成'
    : item.queued
      ? '排队中'
      : item.cancelling
        ? '正在取消'
        : item.preparing
          ? '正在准备'
          : item.kind === 'upload' || item.kind === 'download'
            ? (item.total > 0
                ? Math.round((item.transferred / item.total) * 100) + '%'
                : '正在统计') + (item.speed > 0 ? ' · ' + formatBytes(item.speed) + '/s' : '')
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
        :disabled="!items.some((v) => !v.done)"
        @click="$emit('cancel-all')"
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
        class="relative border-b border-border px-sm py-xs last:border-0 dark:border-border-dark"
      >
        <div class="flex h-[28px] min-w-0 items-center gap-sm">
          <UiTooltip :content="item.remotePath + (item.localPath ? ' → ' + item.localPath : '')">
            <span class="min-w-0 flex-1 truncate text-body-sm">{{ item.label }}</span>
          </UiTooltip>
          <UiTooltip :content="status(item)">
            <span
              class="max-w-[48%] truncate text-caption text-secondary dark:text-secondary-dark"
              >{{ status(item) }}</span
            >
          </UiTooltip>
          <UiIconButton
            v-if="!item.done"
            size="xs"
            label="取消任务"
            :disabled="item.cancelling"
            @click="$emit('cancel', item.id)"
            ><UiIcon name="x" :size="12"
          /></UiIconButton>
          <UiIconButton
            v-else-if="item.error"
            size="xs"
            label="重试任务"
            @click="$emit('retry', item)"
            ><UiIcon name="refresh" :size="12"
          /></UiIconButton>
          <UiIconButton size="xs" label="任务详情" @click="detail = item"
            ><UiIcon name="info" :size="12"
          /></UiIconButton>
        </div>
        <UiProgress
          v-if="!item.done && !item.queued"
          class="!h-[2px]"
          :value="item.total > 0 ? (item.transferred / item.total) * 100 : 0"
          :indeterminate="item.total <= 0"
        />
      </div>
      <slot />
    </UiScrollArea>
  </UiPopover>
  <UiModal :open="!!detail" title="传输详情" @close="detail = undefined">
    <div v-if="detail" class="space-y-sm select-text break-all text-body-sm">
      <p>{{ detail.label }}</p>
      <p>{{ detail.remotePath }}</p>
      <p>{{ detail.localPath }}</p>
      <p>{{ status(detail) }}</p>
      <p>
        {{ formatBytes(detail.transferred) }} /
        {{ detail.total > 0 ? formatBytes(detail.total) : '未知大小' }}
      </p>
      <UiButton size="sm" variant="ghost" @click="$emit('locate', detail)">定位文件</UiButton>
    </div>
  </UiModal>
</template>
