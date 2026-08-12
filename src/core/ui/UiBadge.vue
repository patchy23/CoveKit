<script setup lang="ts">
import type { UiSize, UiTone } from './types'

withDefaults(
  defineProps<{
    tone?: UiTone
    size?: UiSize
  }>(),
  { tone: 'neutral', size: 'sm' }
)
</script>

<template>
  <span class="ui-badge" :class="[`ui-badge-${tone}`, `ui-badge-${size}`]"><slot /></span>
</template>
