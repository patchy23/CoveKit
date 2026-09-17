<script setup lang="ts">
import {
  SelectContent,
  SelectIcon,
  SelectItem,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectViewport,
} from 'reka-ui'
import { computed } from 'vue'
import UiIcon from './UiIcon.vue'
import UiTooltip from './UiTooltip.vue'
import type { UiSize } from './types'

export interface SelectOption {
  value: string
  label?: string
  disabled?: boolean
}

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: SelectOption[]
    title?: string
    placeholder?: string
    disabled?: boolean
    size?: UiSize
    optionClass?: (value: string) => string
    valueClass?: (value: string) => string
  }>(),
  {
    title: '',
    placeholder: '请选择',
    disabled: false,
    size: 'md',
    optionClass: undefined,
    valueClass: undefined,
  }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()

const currentLabel = computed(
  () =>
    props.options.find((option) => option.value === props.modelValue)?.label ??
    props.modelValue ??
    props.placeholder
)
const hasCustomValueColor = computed(() => Boolean(props.valueClass || props.optionClass))

function optionColorClass(value: string) {
  return props.optionClass?.(value) ?? 'text-primary dark:text-primary-dark'
}

function valueColorClass(value: string) {
  return (props.valueClass ?? props.optionClass)?.(value) ?? 'text-primary dark:text-primary-dark'
}
</script>

<template>
  <div class="relative">
    <SelectRoot
      v-slot="{ open }"
      :model-value="modelValue"
      :disabled="disabled"
      @update:model-value="emit('update:modelValue', String($event))"
    >
      <UiTooltip :content="title" :disabled="open">
        <SelectTrigger
          class="flex w-full items-center justify-between gap-[6px] rounded-md border border-border bg-surface px-[10px] outline-none transition-colors hover:border-border-strong focus-visible:border-tertiary disabled:cursor-not-allowed disabled:opacity-60 dark:border-border-dark dark:bg-surface-dark dark:hover:border-border-strong-dark dark:focus-visible:border-tertiary-dark"
          :class="[
            `ui-control-${size}`,
            hasCustomValueColor
              ? valueColorClass(modelValue)
              : 'text-secondary dark:text-secondary-dark',
          ]"
        >
          <SelectValue :placeholder="placeholder" class="truncate">{{ currentLabel }}</SelectValue>
          <SelectIcon as-child>
            <UiIcon
              name="chevron-down"
              :size="12"
              class="shrink-0 transition-transform duration-150"
              :class="{ 'rotate-180': open }"
            />
          </SelectIcon>
        </SelectTrigger>
      </UiTooltip>

      <SelectPortal>
        <SelectContent
          position="popper"
          align="start"
          :side-offset="4"
          class="z-[220] min-w-[var(--reka-select-trigger-width)] overflow-hidden rounded-lg border border-border bg-surface shadow-[0_16px_40px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark"
        >
          <SelectViewport
            class="max-h-[min(280px,var(--reka-select-content-available-height))] overflow-y-auto py-xs"
          >
            <SelectItem
              v-for="option in options"
              :key="option.value"
              :value="option.value"
              :disabled="option.disabled"
              class="flex w-full cursor-default select-none items-center px-[10px] text-left font-medium outline-none transition-colors data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50 data-[highlighted]:bg-border data-[state=checked]:bg-tertiary-soft dark:data-[highlighted]:bg-border-dark dark:data-[state=checked]:bg-tertiary-soft-dark"
              :class="[
                size === 'xs'
                  ? 'py-xs text-caption'
                  : size === 'sm'
                    ? 'py-[6px] text-body-sm'
                    : size === 'lg'
                      ? 'py-[9px] text-body'
                      : 'py-[7px] text-body',
                optionColorClass(option.value),
              ]"
            >
              <SelectItemText>{{ option.label ?? option.value }}</SelectItemText>
            </SelectItem>
          </SelectViewport>
        </SelectContent>
      </SelectPortal>
    </SelectRoot>
  </div>
</template>
