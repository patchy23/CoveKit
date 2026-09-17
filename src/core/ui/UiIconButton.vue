<script setup lang="ts">
import UiButton from './UiButton.vue'
import { computed } from 'vue'
import type { UiSize } from './types'

const props = withDefaults(
  defineProps<{
    label: string
    size?: UiSize
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
    loading?: boolean
  }>(),
  { size: 'md', variant: 'ghost', loading: false }
)

/**
 * 方形按钮内联尺寸（px）
 * 注意：不能用 class 覆盖 UiButton 的 !px-[14px]（特异性 0,2,0 高于单类 0,1,0），
 * 用内联 style 才能确保内容区 = 按钮尺寸，图标不被压缩。
 */
const squareStyle = computed(() => {
  const sizePx = { xs: 24, sm: 28, md: 36, lg: 42 }[props.size]
  return { width: `${sizePx}px`, height: `${sizePx}px`, padding: '0' }
})
</script>

<template>
  <UiButton
    :variant="variant"
    :size="size"
    :loading="loading"
    :style="squareStyle"
    :title="label"
    :aria-label="label"
  >
    <slot />
  </UiButton>
</template>
