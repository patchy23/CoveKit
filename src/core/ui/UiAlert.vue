<script setup lang="ts">
import { cva } from 'class-variance-authority'
import { computed } from 'vue'
import type { UiSize } from './types'
import { cn } from './utils'

const props = withDefaults(
  defineProps<{
    tone?: 'info' | 'success' | 'warning' | 'danger'
    title?: string
    size?: UiSize
  }>(),
  { tone: 'info', title: '', size: 'md' }
)

const alertVariants = cva('ui-alert', {
  variants: {
    tone: {
      info: 'ui-alert-info',
      success: 'ui-alert-success',
      warning: 'ui-alert-warning',
      danger: 'ui-alert-danger',
    },
    size: {
      xs: 'ui-alert-xs',
      sm: 'ui-alert-sm',
      md: 'ui-alert-md',
      lg: 'ui-alert-lg',
    },
  },
})

const classes = computed(() => cn(alertVariants({ tone: props.tone, size: props.size })))
</script>

<template>
  <div :class="classes" role="status">
    <p v-if="title" class="mb-xs font-semibold">{{ title }}</p>
    <div class="leading-relaxed"><slot /></div>
  </div>
</template>
