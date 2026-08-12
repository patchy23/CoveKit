<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: 'text' | 'rect' | 'circle'
    width?: string
    height?: string
    lines?: number
  }>(),
  { variant: 'text', width: '100%', height: '', lines: 1 }
)
</script>

<template>
  <div class="flex flex-col gap-sm" aria-hidden="true">
    <span
      v-for="line in lines"
      :key="line"
      class="block animate-pulse bg-border dark:bg-border-dark"
      :class="
        variant === 'circle'
          ? 'rounded-full'
          : variant === 'rect'
            ? 'rounded-md'
            : 'h-[12px] rounded-full'
      "
      :style="{
        width: line === lines && lines > 1 ? '72%' : width,
        height: height || (variant === 'circle' ? width : variant === 'rect' ? '72px' : undefined),
      }"
    />
  </div>
</template>
