<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
/**
 * UiCombobox · 可输入搜索的下拉选择（core/ui 公共组件）
 * 触发框可直接输入过滤选项；键盘 ↑↓ 高亮、Enter 选中、Esc 收起。
 * 样式与 UiSelect 对齐（ui-control-* 档位 + z-[220] 浮层）；选项过滤大小写不敏感。
 */
import { computed, ref } from 'vue'
import {
  ComboboxAnchor,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxPortal,
  ComboboxRoot,
  ComboboxViewport,
} from 'reka-ui'
import UiIcon from './UiIcon.vue'
import type { UiSize } from './types'

export interface ComboboxOption {
  value: string
  label: string
  /** 搜索时额外匹配的关键词（如掩码摘要、备注） */
  keywords?: string
}

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: ComboboxOption[]
    placeholder?: string
    searchPlaceholder?: string
    emptyText?: string
    disabled?: boolean
    size?: UiSize
  }>(),
  {
    placeholder: '请选择',
    searchPlaceholder: '输入以筛选…',
    emptyText: '无匹配项',
    disabled: false,
    size: 'md',
  }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()

/** 输入框文本（打开时为搜索词；关闭时回显选中项 label） */
const query = ref('')
const open = ref(false)

const selected = computed(() => props.options.find((o) => o.value === props.modelValue))

/** 过滤后的选项（匹配 label 与 keywords，大小写不敏感） */
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return props.options
  return props.options.filter(
    (o) => o.label.toLowerCase().includes(q) || (o.keywords ?? '').toLowerCase().includes(q)
  )
})

function onSelect(value: string) {
  emit('update:modelValue', value)
  open.value = false
  query.value = ''
}

/** 打开时清空搜索词；关闭时回显选中 label 由模板计算属性负责 */
function onOpenChange(v: boolean) {
  open.value = v
  if (v) query.value = ''
}
</script>

<template>
  <ComboboxRoot
    :open="open"
    :model-value="modelValue"
    :disabled="disabled"
    @update:open="onOpenChange"
    @update:model-value="onSelect(String($event))"
  >
    <ComboboxAnchor class="relative">
      <ComboboxInput
        :value="open ? query : (selected?.label ?? '')"
        :placeholder="open ? searchPlaceholder : placeholder"
        class="w-full rounded-md border border-border bg-surface px-[10px] pr-[28px] outline-none transition-colors placeholder:text-text-muted hover:border-border-strong focus:border-tertiary disabled:cursor-not-allowed disabled:opacity-60 dark:border-border-dark dark:bg-surface-dark dark:placeholder:text-text-muted-dark dark:hover:border-border-strong-dark dark:focus:border-tertiary-dark"
        :class="[`ui-control-${size}`, 'text-secondary dark:text-secondary-dark']"
        @input="query = ($event.target as HTMLInputElement).value"
        @focus="onOpenChange(true)"
      />
      <UiIcon
        name="chevron-down"
        :size="12"
        class="pointer-events-none absolute right-[10px] top-1/2 -translate-y-1/2 text-text-muted transition-transform duration-150 dark:text-text-muted-dark"
        :class="{ 'rotate-180': open }"
      />
    </ComboboxAnchor>

    <ComboboxPortal>
      <ComboboxContent
        position="popper"
        align="start"
        :side-offset="4"
        class="z-[220] w-[var(--reka-combobox-trigger-width)] overflow-hidden rounded-lg border border-border bg-surface shadow-[0_16px_40px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark"
      >
        <UiScrollArea as-child axis="vertical">
          <ComboboxViewport
            class="max-h-[min(280px,var(--reka-combobox-content-available-height))] py-xs"
          >
            <ComboboxEmpty
              class="px-[10px] py-[8px] text-body-sm text-text-muted dark:text-text-muted-dark"
            >
              {{ emptyText }}
            </ComboboxEmpty>
            <ComboboxItem
              v-for="option in filtered"
              :key="option.value"
              :value="option.value"
              class="flex w-full cursor-default select-none items-center px-[10px] text-left font-medium text-secondary outline-none transition-colors data-[highlighted]:bg-border data-[state=checked]:bg-tertiary-soft data-[state=checked]:text-tertiary-strong dark:text-secondary-dark dark:data-[highlighted]:bg-border-dark dark:data-[state=checked]:bg-tertiary-soft-dark dark:data-[state=checked]:text-tertiary-dark"
              :class="[
                size === 'xs'
                  ? 'py-xs text-caption'
                  : size === 'sm'
                    ? 'py-[6px] text-body-sm'
                    : size === 'lg'
                      ? 'py-[9px] text-body'
                      : 'py-[7px] text-body',
              ]"
            >
              {{ option.label }}
            </ComboboxItem>
          </ComboboxViewport>
        </UiScrollArea>
      </ComboboxContent>
    </ComboboxPortal>
  </ComboboxRoot>
</template>
