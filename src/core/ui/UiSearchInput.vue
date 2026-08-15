<script setup lang="ts">
import UiIcon from './UiIcon.vue'
import UiInput from './UiInput.vue'
import type { UiSize } from './types'

withDefaults(
  defineProps<{
    modelValue: string
    placeholder?: string
    size?: UiSize
    disabled?: boolean
    clearable?: boolean
  }>(),
  { placeholder: '搜索…', size: 'md', disabled: false, clearable: true }
)
const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()
</script>

<template>
  <div class="relative">
    <UiIcon
      name="search"
      :size="14"
      class="pointer-events-none absolute left-[10px] top-1/2 -translate-y-1/2 text-text-muted"
    />
    <UiInput
      :model-value="modelValue"
      :size="size"
      :placeholder="placeholder"
      :disabled="disabled"
      class="ui-search-control"
      type="search"
      @update:model-value="emit('update:modelValue', String($event))"
    />
    <button
      v-if="clearable && modelValue && !disabled"
      type="button"
      class="absolute right-[8px] top-1/2 grid h-[18px] w-[18px] -translate-y-1/2 place-items-center rounded-full text-caption text-text-muted hover:bg-border hover:text-primary dark:hover:bg-border-dark dark:hover:text-primary-dark"
      aria-label="清空搜索"
      @click="emit('update:modelValue', '')"
    >
      ×
    </button>
  </div>
</template>
