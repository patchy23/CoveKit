<script setup lang="ts">
/**
 * UiStatusBar · 区块底部状态栏
 * 统一形态：flex shrink-0 items-center border-t px-[12px] py-[6px] text-caption text-text-muted。
 * 左区放默认插槽（计数、路径等），右区放 #trailing（自动 ml-auto，连接状态等）。
 * 传 tone 时在最左渲染状态点（neutral/success/danger）；不传不渲染（纯信息栏）。
 * size：sm(24px，默认，计数+状态摘要)；md(36px，文件管理器等需要路径展示的场景)。
 */
withDefaults(defineProps<{ tone?: 'neutral' | 'success' | 'danger'; size?: 'sm' | 'md' }>(), {
  tone: undefined,
  size: 'sm',
})
</script>

<template>
  <div
    class="flex shrink-0 items-center gap-[12px] overflow-hidden border-t border-border bg-surface-muted px-[12px] text-caption text-text-muted dark:border-border-dark dark:bg-surface-muted-dark dark:text-text-muted-dark"
    :class="size === 'md' ? 'h-[36px]' : 'h-[24px]'"
  >
    <span
      v-if="tone"
      class="h-[7px] w-[7px] shrink-0 rounded-full"
      :class="{
        'bg-text-muted dark:bg-text-muted-dark': tone === 'neutral',
        'bg-success-strong dark:bg-success-dark': tone === 'success',
        'bg-danger-strong dark:bg-danger-dark': tone === 'danger',
      }"
    />
    <slot />
    <div v-if="$slots.trailing" class="ml-auto flex items-center gap-[12px]">
      <slot name="trailing" />
    </div>
  </div>
</template>
