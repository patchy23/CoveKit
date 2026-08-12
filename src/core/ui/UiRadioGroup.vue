<script setup lang="ts">
import { RadioGroupIndicator, RadioGroupItem, RadioGroupRoot } from 'reka-ui'
import type { UiSize } from './types'

export interface RadioOption {
  value: string
  label: string
  description?: string
  disabled?: boolean
}

withDefaults(
  defineProps<{
    modelValue: string
    options: RadioOption[]
    name: string
    size?: UiSize
    direction?: 'row' | 'column'
    disabled?: boolean
  }>(),
  { size: 'md', direction: 'column', disabled: false }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()
</script>

<template>
  <RadioGroupRoot
    :model-value="modelValue"
    :name="name"
    :disabled="disabled"
    class="flex gap-md"
    :class="direction === 'column' ? 'flex-col' : 'flex-row flex-wrap'"
    @update:model-value="emit('update:modelValue', String($event))"
  >
    <label
      v-for="option in options"
      :key="option.value"
      class="inline-flex items-start gap-sm"
      :class="disabled || option.disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'"
    >
      <RadioGroupItem
        :value="option.value"
        class="mt-[2px] grid shrink-0 place-items-center rounded-full border border-border-strong bg-surface outline-none transition-colors data-[state=checked]:border-tertiary-strong focus-visible:ring-2 focus-visible:ring-tertiary/30 dark:border-border-strong-dark dark:bg-surface-dark dark:data-[state=checked]:border-tertiary-dark"
        :class="
          size === 'xs'
            ? 'h-[12px] w-[12px]'
            : size === 'sm'
              ? 'h-[14px] w-[14px]'
              : size === 'lg'
                ? 'h-[18px] w-[18px]'
                : 'h-4 w-4'
        "
        :disabled="disabled || option.disabled"
        :aria-label="option.label"
      >
        <RadioGroupIndicator
          class="h-1/2 w-1/2 rounded-full bg-tertiary-strong dark:bg-tertiary-dark"
        />
      </RadioGroupItem>
      <span>
        <span
          class="block font-medium text-primary dark:text-primary-dark"
          :class="size === 'xs' ? 'text-caption' : size === 'sm' ? 'text-body-sm' : 'text-body'"
          >{{ option.label }}</span
        >
        <span
          v-if="option.description"
          class="mt-[2px] block text-caption text-text-muted dark:text-text-muted-dark"
          >{{ option.description }}</span
        >
      </span>
    </label>
  </RadioGroupRoot>
</template>
