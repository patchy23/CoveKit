<script setup lang="ts">
import { TabsList, TabsRoot, TabsTrigger } from 'reka-ui'
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
  <TabsRoot
    :model-value="modelValue"
    activation-mode="manual"
    @update:model-value="emit('update:modelValue', String($event))"
  >
    <TabsList class="ui-tabs" :class="`ui-tabs-${variant}`">
      <TabsTrigger
        v-for="item in items"
        :key="item.value"
        :value="item.value"
        class="ui-tab"
        :class="[`ui-tab-${size}`, { 'ui-tab-active': item.value === modelValue }]"
        :disabled="item.disabled"
      >
        {{ item.label }}
        <span v-if="item.badge !== undefined" class="ui-tab-badge">{{ item.badge }}</span>
      </TabsTrigger>
    </TabsList>
  </TabsRoot>
</template>
