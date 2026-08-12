<script setup lang="ts">
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
  <div class="flex gap-md" :class="direction === 'column' ? 'flex-col' : 'flex-row flex-wrap'">
    <label
      v-for="option in options"
      :key="option.value"
      class="inline-flex items-start gap-sm"
      :class="disabled || option.disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'"
    >
      <input
        type="radio"
        class="mt-[2px] accent-[var(--color-tertiary-strong)]"
        :class="
          size === 'xs'
            ? 'h-[12px] w-[12px]'
            : size === 'sm'
              ? 'h-[14px] w-[14px]'
              : size === 'lg'
                ? 'h-[18px] w-[18px]'
                : 'h-4 w-4'
        "
        :name="name"
        :value="option.value"
        :checked="modelValue === option.value"
        :disabled="disabled || option.disabled"
        @change="emit('update:modelValue', option.value)"
      />
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
  </div>
</template>
