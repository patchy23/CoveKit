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
    variant?: 'radio' | 'chips'
  }>(),
  { size: 'md', direction: 'column', disabled: false, variant: 'radio' }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()
</script>

<template>
  <RadioGroupRoot
    :model-value="modelValue"
    :name="name"
    :disabled="disabled"
    :orientation="variant === 'chips' || direction === 'row' ? 'horizontal' : 'vertical'"
    class="flex"
    :class="[
      variant === 'chips' || direction === 'row' ? 'flex-row flex-wrap' : 'flex-col',
      variant === 'chips' ? 'gap-xs' : 'gap-md',
    ]"
    @update:model-value="emit('update:modelValue', String($event))"
  >
    <template v-if="variant === 'chips'">
      <RadioGroupItem
        v-for="option in options"
        :key="option.value"
        :value="option.value"
        :disabled="disabled || option.disabled"
        :aria-label="option.label"
        class="rounded-full border border-border px-sm text-secondary outline-none transition-colors hover:bg-border focus-visible:ring-2 focus-visible:ring-tertiary/30 disabled:cursor-not-allowed disabled:opacity-60 data-[state=checked]:border-tertiary-strong data-[state=checked]:bg-tertiary-soft data-[state=checked]:text-tertiary-strong data-[state=checked]:hover:bg-tertiary-soft dark:border-border-dark dark:text-secondary-dark dark:hover:bg-border-dark dark:data-[state=checked]:border-tertiary-dark dark:data-[state=checked]:bg-tertiary-soft-dark dark:data-[state=checked]:text-tertiary-dark dark:data-[state=checked]:hover:bg-tertiary-soft-dark"
        :class="[
          `ui-control-${size}`,
          size === 'xs' ? 'text-caption' : size === 'sm' ? 'text-body-sm' : 'text-body',
        ]"
        >{{ option.label }}</RadioGroupItem
      >
    </template>
    <template v-else>
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
    </template>
  </RadioGroupRoot>
</template>
