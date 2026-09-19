<script setup lang="ts">
/**
 * UiTabStatusDot · 页签状态点（UiTabs / UiTabsOverflowMenu 共享的内部子组件，不经入口导出）
 * 颜色语义：success=已连接 danger=已断开 neutral=中性 progress=进行中（脉动）。
 */
import UiTooltip from './UiTooltip.vue'
import type { UiTabItem } from './UiTabs.vue'

const props = defineProps<{
  status: NonNullable<UiTabItem['status']>
  /** 状态点 tooltip（默认按连接语义：已连接/已断开/连接中） */
  title?: string
}>()

/** 默认文案回退（与 UiTabs 原逻辑一致，两处复用后不再漂移） */
function fallbackTitle(status: NonNullable<UiTabItem['status']>): string {
  if (status === 'success') return '已连接'
  if (status === 'danger') return '已断开'
  return '连接中'
}
</script>

<template>
  <UiTooltip :content="props.title ?? fallbackTitle(props.status)">
    <span
      class="h-[7px] w-[7px] shrink-0 rounded-full"
      :class="{
        'bg-success-strong dark:bg-success-dark': props.status === 'success',
        'bg-danger-strong dark:bg-danger-dark': props.status === 'danger',
        'bg-text-muted dark:bg-text-muted-dark': props.status === 'neutral',
        'bg-tertiary animate-pulse dark:bg-tertiary-dark': props.status === 'progress',
      }"
      aria-hidden="true"
    />
  </UiTooltip>
</template>
