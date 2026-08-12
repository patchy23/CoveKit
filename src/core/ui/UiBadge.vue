<script setup lang="ts">
import { cva } from 'class-variance-authority'
import { computed } from 'vue'
import type { UiSize, UiTone } from './types'
import { cn } from './utils'

const props = withDefaults(
  defineProps<{
    tone?: UiTone
    size?: UiSize
  }>(),
  { tone: 'neutral', size: 'sm' }
)

const badgeVariants = cva('ui-badge', {
  variants: {
    tone: {
      neutral: 'ui-badge-neutral',
      accent: 'ui-badge-accent',
      success: 'ui-badge-success',
      warning: 'ui-badge-warning',
      danger: 'ui-badge-danger',
      info: 'ui-badge-info',
      purple: 'ui-badge-purple',
    },
    size: {
      xs: 'ui-badge-xs',
      sm: 'ui-badge-sm',
      md: 'ui-badge-md',
      lg: 'ui-badge-lg',
    },
  },
})

const classes = computed(() => cn(badgeVariants({ tone: props.tone, size: props.size })))
</script>

<template>
  <span :class="classes"><slot /></span>
</template>
