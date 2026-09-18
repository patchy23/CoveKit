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
        // circle 的高度只在显式给了 height 时采用，否则跟随 width；width 为百分比时高度不能照抄（会得到椭圆），回退为正方形边长语义交给 aspect-ratio
        height:
          height ||
          (variant === 'circle'
            ? width.endsWith('px')
              ? width
              : undefined
            : variant === 'rect'
              ? '72px'
              : undefined),
        aspectRatio: variant === 'circle' && !height ? '1' : undefined,
      }"
    />
  </div>
</template>
