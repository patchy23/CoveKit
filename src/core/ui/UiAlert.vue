<script setup lang="ts">
withDefaults(
  defineProps<{
    tone?: 'info' | 'success' | 'warning' | 'danger'
    title?: string
  }>(),
  { tone: 'info', title: '' }
)
</script>

<template>
  <div class="ui-alert" :class="`ui-alert-${tone}`" role="status">
    <p v-if="title" class="mb-xs font-semibold">{{ title }}</p>
    <div class="text-body-sm leading-relaxed"><slot /></div>
  </div>
</template>
