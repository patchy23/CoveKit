<script setup lang="ts">
import type { UiSize } from './types'

withDefaults(defineProps<{ size?: UiSize; label?: string }>(), { size: 'md', label: '加载中' })
</script>

<template>
  <span class="inline-flex items-center gap-sm" role="status" :aria-label="label">
    <span
      class="animate-spin rounded-full border-2 border-current border-r-transparent"
      :class="
        size === 'xs'
          ? 'h-[10px] w-[10px]'
          : size === 'sm'
            ? 'h-[12px] w-[12px]'
            : size === 'lg'
              ? 'h-[20px] w-[20px]'
              : 'h-4 w-4'
      "
      aria-hidden="true"
    />
    <span
      v-if="$slots.default"
      :class="size === 'xs' ? 'text-caption' : size === 'sm' ? 'text-body-sm' : 'text-body'"
      ><slot
    /></span>
  </span>
</template>
