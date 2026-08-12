<script setup lang="ts">
import { computed } from 'vue'
import UiButton from './UiButton.vue'
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

const squareClass = computed(
  () =>
    ({
      xs: '!w-[24px] !px-0',
      sm: '!w-[28px] !px-0',
      md: '!w-[36px] !px-0',
      lg: '!w-[42px] !px-0',
    })[props.size]
)
</script>

<template>
  <UiButton
    :variant="variant"
    :size="size"
    :loading="loading"
    :class="squareClass"
    :title="label"
    :aria-label="label"
  >
    <slot />
  </UiButton>
</template>
