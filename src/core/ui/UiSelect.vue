<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
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
import { computed, watchEffect } from 'vue'
import UiIcon from './UiIcon.vue'
import UiTooltip from './UiTooltip.vue'
import type { UiSize } from './types'
import { UI_FLOATING_PANEL_CLASS, uiOptionSizeClass } from './utils'

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

// 空串 value 会让 reka 弹层渲染即崩（11 号文红线）；dev 下给出显式告警而不是静默炸
if (import.meta.env.DEV) {
  watchEffect(() => {
    if (props.options.some((o) => o.value === '')) {
      console.warn(
        '[UiSelect] 选项 value 不能为空串：「不选/跟随默认」请用非空哨兵值并在选中回调里反映射'
      )
    }
  })
}

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
      <!-- Select 的锚点必须位于 TooltipRoot 外，避免被提示浮层的 PopperRoot 截获。 -->
      <SelectTrigger as-child>
        <UiTooltip :content="title" :disabled="open">
          <button
            type="button"
            :disabled="disabled"
            class="flex w-full items-center justify-between gap-[6px] rounded-md border border-border bg-surface px-[10px] text-secondary outline-none transition-colors hover:border-border-strong focus-visible:border-tertiary disabled:cursor-not-allowed disabled:opacity-60 dark:border-border-dark dark:bg-surface-dark dark:text-secondary-dark dark:hover:border-border-strong-dark dark:focus-visible:border-tertiary-dark"
            :class="`ui-control-${size}`"
          >
            <SelectValue
              :placeholder="placeholder"
              class="truncate"
              :class="hasCustomValueColor ? valueColorClass(modelValue) : undefined"
              >{{ currentLabel }}</SelectValue
            >
            <SelectIcon as-child>
              <UiIcon
                name="chevron-down"
                :size="12"
                class="shrink-0 transition-transform duration-150"
                :class="{ 'rotate-180': open }"
              />
            </SelectIcon>
          </button>
        </UiTooltip>
      </SelectTrigger>

      <SelectPortal>
        <SelectContent
          position="popper"
          align="start"
          :side-offset="4"
          :class="[UI_FLOATING_PANEL_CLASS, 'min-w-[var(--reka-select-trigger-width)]']"
        >
          <UiScrollArea as-child axis="vertical">
            <SelectViewport
              class="max-h-[min(280px,var(--reka-select-content-available-height))] py-xs"
            >
              <SelectItem
                v-for="option in options"
                :key="option.value"
                :value="option.value"
                :disabled="option.disabled"
                class="flex w-full cursor-default select-none items-center px-[10px] text-left font-medium outline-none transition-colors data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50 data-[highlighted]:bg-border data-[state=checked]:bg-tertiary-soft dark:data-[highlighted]:bg-border-dark dark:data-[state=checked]:bg-tertiary-soft-dark"
                :class="[uiOptionSizeClass(size), optionColorClass(option.value)]"
              >
                <SelectItemText>{{ option.label ?? option.value }}</SelectItemText>
              </SelectItem>
            </SelectViewport>
          </UiScrollArea>
        </SelectContent>
      </SelectPortal>
    </SelectRoot>
  </div>
</template>
