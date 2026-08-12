<script setup lang="ts">
import type { UiSize } from './types'

withDefaults(
  defineProps<{
    tone?: 'info' | 'success' | 'warning' | 'danger'
    title?: string
    size?: UiSize
  }>(),
  { tone: 'info', title: '', size: 'md' }
)
</script>

<template>
  <div class="ui-alert" :class="[`ui-alert-${tone}`, `ui-alert-${size}`]" role="status">
    <p v-if="title" class="mb-xs font-semibold">{{ title }}</p>
    <div class="leading-relaxed"><slot /></div>
  </div>
</template>
