<script setup lang="ts">
import { CheckboxIndicator, CheckboxRoot } from 'reka-ui'
import { computed } from 'vue'
import type { UiSize } from './types'

const props = withDefaults(
  defineProps<{
    modelValue: boolean
    label?: string
    description?: string
    disabled?: boolean
    indeterminate?: boolean
    size?: UiSize
  }>(),
  { label: '', description: '', disabled: false, indeterminate: false, size: 'md' }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: boolean): void }>()
const boxClass = computed(
  () =>
    ({ xs: 'h-[12px] w-[12px]', sm: 'h-[14px] w-[14px]', md: 'h-4 w-4', lg: 'h-[18px] w-[18px]' })[
      props.size
    ]
)
const textClass = computed(() =>
  props.size === 'xs' ? 'text-caption' : props.size === 'sm' ? 'text-body-sm' : 'text-body'
)

function updateValue(value: boolean | 'indeterminate') {
  emit('update:modelValue', value === 'indeterminate' ? true : value)
}
</script>

<template>
  <label
    class="inline-flex items-start gap-sm"
    :class="disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'"
  >
    <CheckboxRoot
      :model-value="indeterminate ? 'indeterminate' : modelValue"
      class="mt-[2px] grid shrink-0 place-items-center rounded-[3px] border border-border-strong bg-surface text-on-tertiary outline-none transition-colors data-[state=checked]:border-tertiary-strong data-[state=checked]:bg-tertiary-strong data-[state=indeterminate]:border-tertiary-strong data-[state=indeterminate]:bg-tertiary-strong focus-visible:ring-2 focus-visible:ring-tertiary/30 dark:border-border-strong-dark dark:bg-surface-dark dark:data-[state=checked]:border-tertiary-dark dark:data-[state=checked]:bg-tertiary-dark dark:data-[state=checked]:text-on-tertiary-dark dark:data-[state=indeterminate]:border-tertiary-dark dark:data-[state=indeterminate]:bg-tertiary-dark"
      :class="boxClass"
      :disabled="disabled"
      :aria-label="label || undefined"
      @update:model-value="updateValue"
    >
      <CheckboxIndicator class="grid place-items-center">
        <svg v-if="indeterminate" width="8" height="8" viewBox="0 0 8 8" aria-hidden="true">
          <path d="M1 4h6" fill="none" stroke="currentColor" stroke-width="1.5" />
        </svg>
        <svg v-else width="9" height="9" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M2 5l2 2 4-4" fill="none" stroke="currentColor" stroke-width="1.5" />
        </svg>
      </CheckboxIndicator>
    </CheckboxRoot>
    <span v-if="label || description" class="min-w-0">
      <span
        v-if="label"
        class="block font-medium text-primary dark:text-primary-dark"
        :class="textClass"
        >{{ label }}</span
      >
      <span
        v-if="description"
        class="mt-[2px] block text-caption text-text-muted dark:text-text-muted-dark"
        >{{ description }}</span
      >
    </span>
  </label>
</template>
