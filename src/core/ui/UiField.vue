<script setup lang="ts">
import type { UiSize } from './types'

defineProps<{
  label?: string
  description?: string
  error?: string
  required?: boolean
  size?: UiSize
}>()
</script>

<template>
  <label class="flex min-w-0 flex-col gap-xs">
    <span
      v-if="label"
      class="field-label"
      :class="size === 'xs' ? 'text-caption' : size === 'sm' ? 'text-body-sm' : 'text-body'"
    >
      {{ label }}<span v-if="required" class="text-danger-strong"> *</span>
    </span>
    <slot />
    <span v-if="error" class="text-caption text-danger-strong dark:text-danger-dark">
      {{ error }}
    </span>
    <span v-else-if="description" class="text-caption text-text-muted dark:text-text-muted-dark">
      {{ description }}
    </span>
  </label>
</template>
