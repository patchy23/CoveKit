<script setup lang="ts">
withDefaults(defineProps<{ orientation?: 'horizontal' | 'vertical'; label?: string }>(), {
  orientation: 'horizontal',
  label: '',
})
</script>

<template>
  <div
    v-if="orientation === 'vertical'"
    class="h-full min-h-[20px] w-px bg-border dark:bg-border-dark"
    role="separator"
    aria-orientation="vertical"
  />
  <div v-else class="flex items-center gap-md" role="separator">
    <span class="h-px flex-1 bg-border dark:bg-border-dark" />
    <span v-if="label" class="text-caption text-text-muted dark:text-text-muted-dark">{{
      label
    }}</span>
    <span v-if="label" class="h-px flex-1 bg-border dark:bg-border-dark" />
  </div>
</template>
