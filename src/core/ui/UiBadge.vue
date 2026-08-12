<script setup lang="ts">
withDefaults(
  defineProps<{
    tone?: 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info' | 'purple'
  }>(),
  { tone: 'neutral' }
)
</script>

<template>
  <span class="ui-badge" :class="`ui-badge-${tone}`"><slot /></span>
</template>
