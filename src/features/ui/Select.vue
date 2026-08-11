<script setup lang="ts">
/**
 * Select · 通用下拉选择组件（替代原生 select，贴合 patchyBox UI 风格）
 * - 触发按钮与 field-input 同款；下拉面板圆角/边框/阴影与全局一致
 * - 选项 hover 用 bg-border（与列表项同款），选中项文字高亮
 * - optionClass / valueClass 允许按值定制颜色（如 HTTP 方法色标）
 */
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

export interface SelectOption {
  value: string
  label?: string
}

const props = withDefaults(
  defineProps<{
    modelValue: string
    options: SelectOption[]
    title?: string
    disabled?: boolean
    /** 尺寸：md=标准（36px，与 field-input 对齐）；sm=紧凑（28px，页签工具栏筛选） */
    size?: 'md' | 'sm'
    /** 下拉选项文字色（按值） */
    // eslint-disable-next-line vue/require-default-prop -- 函数类型可选，模板已空值安全
    optionClass?: (value: string) => string
    /** 值区文字色（默认同 optionClass） */
    // eslint-disable-next-line vue/require-default-prop -- 函数类型可选，模板已空值安全
    valueClass?: (value: string) => string
  }>(),
  { title: '', disabled: false, size: 'md' }
)

const emit = defineEmits<{ (e: 'update:modelValue', v: string): void }>()

const open = ref(false)
/** 向上弹出（下方空间不足时翻转，避免触发页面滚动条） */
const flipUp = ref(false)
/** 面板最大高度（按可用空间收缩，默认 280） */
const panelMaxH = ref(280)

const currentLabel = () =>
  props.options.find((o) => o.value === props.modelValue)?.label ?? props.modelValue

const clsFor = (v: string) =>
  (props.valueClass ?? props.optionClass)?.(v) ?? 'text-primary dark:text-primary-dark'

function toggle() {
  if (props.disabled) return
  open.value = !open.value
  if (open.value) {
    // 打开时测量空间：下方不足 280 且上方更多 → 向上弹出
    nextTick(() => {
      const rect = triggerEl.value?.getBoundingClientRect()
      if (!rect) return
      const below = window.innerHeight - rect.bottom
      const above = rect.top
      flipUp.value = below < 280 && above > below
      panelMaxH.value = Math.max(120, Math.min(280, (flipUp.value ? above : below) - 8))
    })
  }
}

function select(v: string) {
  emit('update:modelValue', v)
  open.value = false
}

function onDocMouseDown(e: MouseEvent) {
  if (!open.value) return
  const el = triggerEl.value
  if (el && !el.contains(e.target as Node)) open.value = false
}

const triggerEl = ref<HTMLElement | null>(null)

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') open.value = false
}

onMounted(() => {
  document.addEventListener('mousedown', onDocMouseDown)
})

onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onDocMouseDown)
})
</script>

<template>
  <div ref="triggerEl" class="relative select-none">
    <!-- 触发按钮（field-input 同款外观；sm 尺寸用于页签工具栏筛选） -->
    <button
      type="button"
      class="flex w-full items-center justify-between gap-[6px] rounded-md border border-border bg-surface px-[10px] transition-colors hover:border-border-strong disabled:cursor-not-allowed disabled:opacity-60 dark:border-border-dark dark:bg-surface-dark dark:hover:border-border-strong-dark"
      :class="[
        props.size === 'sm' ? 'h-[28px] text-caption' : 'py-[8px] text-body font-medium',
        clsFor(modelValue),
        open ? 'border-tertiary dark:border-tertiary-dark' : '',
      ]"
      :title="title"
      :disabled="disabled"
      @click="toggle"
      @keydown="onKeydown"
    >
      <span class="truncate">{{ currentLabel() }}</span>
      <svg
        class="shrink-0 transition-transform duration-150"
        :class="open ? 'rotate-180' : ''"
        width="12"
        height="12"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M6 9l6 6 6-6" />
      </svg>
    </button>

    <!-- 下拉面板（下方空间不足时向上弹出） -->
    <div
      v-if="open"
      class="absolute left-0 z-50 w-full overflow-y-auto rounded-lg border border-border bg-surface py-[4px] shadow-[0_16px_40px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark"
      :class="flipUp ? 'bottom-full mb-[4px]' : 'top-full mt-[4px]'"
      :style="{ maxHeight: panelMaxH + 'px' }"
    >
      <button
        v-for="opt in options"
        :key="opt.value"
        type="button"
        class="flex w-full items-center px-[10px] py-[7px] text-left font-medium transition-colors hover:bg-border dark:hover:bg-border-dark"
        :class="[
          props.size === 'sm' ? 'text-caption' : 'text-body',
          clsFor(opt.value),
          opt.value === modelValue ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark' : '',
        ]"
        @click="select(opt.value)"
      >
        {{ opt.label ?? opt.value }}
      </button>
    </div>
  </div>
</template>
