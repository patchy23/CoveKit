<script setup lang="ts">
/**
 * FileTransferStrip · 文件页底部传输队列条（P4 将被进度按钮+上拉面板取代，先独立成组件）
 */
import type { TransferItem } from './useFileTransfer'
import { UiButton } from '@/core/ui'

defineProps<{
  /** 进行中或出错的传输项 */
  items: TransferItem[]
}>()

const emit = defineEmits<{
  (e: 'cancel', id: string): void
}>()
</script>

<template>
  <div
    v-if="items.length"
    class="flex max-h-[110px] shrink-0 flex-col gap-[4px] overflow-y-auto border-t border-border px-[12px] py-[6px] dark:border-border-dark"
  >
    <div v-for="item in items" :key="item.id" class="flex items-center gap-[8px] text-caption">
      <span
        class="shrink-0 rounded-full bg-neutral px-[7px] py-[1px] font-medium text-text-muted dark:bg-neutral-dark dark:text-text-muted-dark"
      >
        {{ item.kind === 'upload' ? '上传' : '下载' }}
      </span>
      <span class="min-w-0 flex-1 truncate text-secondary dark:text-secondary-dark">{{
        item.label
      }}</span>
      <span v-if="item.total > 0" class="shrink-0 font-mono text-text-muted">
        {{ Math.min(100, Math.round((item.transferred / item.total) * 100)) }}%
      </span>
      <span v-if="item.error" class="shrink-0 text-danger-strong dark:text-danger-dark">{{
        item.error
      }}</span>
      <UiButton
        v-if="!item.done"
        variant="ghost"
        size="xs"
        class="!h-auto !px-[6px] !py-[1px] text-caption text-danger-strong dark:text-danger-dark"
        title="取消传输"
        @click="emit('cancel', item.id)"
      >
        取消
      </UiButton>
    </div>
  </div>
</template>
