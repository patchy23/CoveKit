<script setup lang="ts">
import type { UiSize } from './types'

export interface UiTabItem {
  value: string
  label: string
  disabled?: boolean
  badge?: string | number
}

withDefaults(
  defineProps<{
    modelValue: string
    items: UiTabItem[]
    variant?: 'pill' | 'line'
    size?: UiSize
  }>(),
  { variant: 'pill', size: 'md' }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()
</script>

<template>
  <div class="ui-tabs" :class="`ui-tabs-${variant}`" role="tablist">
    <button
      v-for="item in items"
      :key="item.value"
      type="button"
      class="ui-tab"
      :class="[`ui-tab-${size}`, { 'ui-tab-active': item.value === modelValue }]"
      :disabled="item.disabled"
      :aria-selected="item.value === modelValue"
      role="tab"
      @click="emit('update:modelValue', item.value)"
    >
      {{ item.label }}
      <span v-if="item.badge !== undefined" class="ui-tab-badge">{{ item.badge }}</span>
    </button>
  </div>
</template>
