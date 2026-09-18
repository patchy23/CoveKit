<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
/**
 * UiCombobox · 可输入搜索的下拉选择（core/ui 公共组件）
 * 触发框可直接输入过滤选项；键盘 ↑↓ 高亮、Enter 选中、Esc 收起。
 * 样式与 UiSelect 对齐（ui-control-* 档位 + z-[220] 浮层）；选项过滤大小写不敏感。
 */
import { computed, ref, watchEffect } from 'vue'
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
import UiTooltip from './UiTooltip.vue'
import type { UiSize } from './types'
import { UI_FLOATING_PANEL_CLASS, uiOptionSizeClass } from './utils'

export interface ComboboxOption {
  value: string
  /** 展示文案（缺省回退 value） */
  label?: string
  /** 禁用该项（渲染为不可选） */
  disabled?: boolean
  /** 搜索时额外匹配的关键词（如掩码摘要、备注） */
  keywords?: string
}

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: ComboboxOption[]
    /** 触发器 tooltip */
    title?: string
    placeholder?: string
    searchPlaceholder?: string
    emptyText?: string
    disabled?: boolean
    size?: UiSize
  }>(),
  {
    title: '',
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

/** 选项展示文案（label 缺省回退 value） */
function optionLabel(option: ComboboxOption): string {
  return option.label ?? option.value
}

/** 过滤后的选项（匹配 label 与 keywords，大小写不敏感） */
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return props.options
  return props.options.filter(
    (o) => optionLabel(o).toLowerCase().includes(q) || (o.keywords ?? '').toLowerCase().includes(q)
  )
})

// 空串 value 会让 reka 弹层渲染即崩（11 号文红线）；dev 下给出显式告警而不是静默炸
if (import.meta.env.DEV) {
  watchEffect(() => {
    if (props.options.some((o) => o.value === '')) {
      console.warn(
        '[UiCombobox] 选项 value 不能为空串：「不选/跟随默认」请用非空哨兵值并在选中回调里反映射'
      )
    }
  })
}

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
      <UiTooltip :content="title" :disabled="open">
        <ComboboxInput
          autocomplete="off"
          :value="open ? query : selected ? optionLabel(selected) : ''"
          :placeholder="open ? searchPlaceholder : placeholder"
          class="w-full rounded-md border border-border bg-surface px-[10px] pr-[28px] outline-none transition-colors placeholder:text-text-muted hover:border-border-strong focus:border-tertiary disabled:cursor-not-allowed disabled:opacity-60 dark:border-border-dark dark:bg-surface-dark dark:placeholder:text-text-muted-dark dark:hover:border-border-strong-dark dark:focus:border-tertiary-dark"
          :class="[`ui-control-${size}`, 'text-secondary dark:text-secondary-dark']"
          @input="query = ($event.target as HTMLInputElement).value"
          @focus="onOpenChange(true)"
        />
      </UiTooltip>
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
        :class="[UI_FLOATING_PANEL_CLASS, 'w-[var(--reka-combobox-trigger-width)]']"
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
              :disabled="option.disabled"
              class="flex w-full cursor-default select-none items-center px-[10px] text-left font-medium text-secondary outline-none transition-colors data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50 data-[highlighted]:bg-border data-[state=checked]:bg-tertiary-soft data-[state=checked]:text-tertiary-strong dark:text-secondary-dark dark:data-[highlighted]:bg-border-dark dark:data-[state=checked]:bg-tertiary-soft-dark dark:data-[state=checked]:text-tertiary-dark"
              :class="uiOptionSizeClass(size)"
            >
              {{ optionLabel(option) }}
            </ComboboxItem>
          </ComboboxViewport>
        </UiScrollArea>
      </ComboboxContent>
    </ComboboxPortal>
  </ComboboxRoot>
</template>
