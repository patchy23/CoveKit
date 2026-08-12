<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'

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
    size?: 'sm' | 'md'
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
const root = ref<HTMLElement | null>(null)
const open = ref(false)
const flipUp = ref(false)
const panelMaxHeight = ref(280)

const currentLabel = computed(
  () =>
    props.options.find((option) => option.value === props.modelValue)?.label ??
    props.modelValue ??
    props.placeholder
)

function colorClass(value: string) {
  return (props.valueClass ?? props.optionClass)?.(value) ?? 'text-primary dark:text-primary-dark'
}

async function toggle() {
  if (props.disabled) return
  open.value = !open.value
  if (!open.value) return
  await nextTick()
  const rect = root.value?.getBoundingClientRect()
  if (!rect) return
  const below = window.innerHeight - rect.bottom
  const above = rect.top
  flipUp.value = below < 280 && above > below
  panelMaxHeight.value = Math.max(120, Math.min(280, (flipUp.value ? above : below) - 8))
}

function select(option: SelectOption) {
  if (option.disabled) return
  emit('update:modelValue', option.value)
  open.value = false
}

function onDocumentMouseDown(event: MouseEvent) {
  if (open.value && root.value && !root.value.contains(event.target as Node)) open.value = false
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') open.value = false
}

onMounted(() => document.addEventListener('mousedown', onDocumentMouseDown))
onBeforeUnmount(() => document.removeEventListener('mousedown', onDocumentMouseDown))
</script>

<template>
  <div ref="root" class="relative select-none">
    <button
      type="button"
      class="flex w-full items-center justify-between gap-[6px] rounded-md border border-border bg-surface px-[10px] transition-colors hover:border-border-strong disabled:cursor-not-allowed disabled:opacity-60 dark:border-border-dark dark:bg-surface-dark dark:hover:border-border-strong-dark"
      :class="[
        size === 'sm' ? 'h-[30px] text-caption' : 'h-[36px] text-body font-medium',
        colorClass(modelValue),
        open ? 'border-tertiary dark:border-tertiary-dark' : '',
      ]"
      :title="title"
      :disabled="disabled"
      :aria-expanded="open"
      aria-haspopup="listbox"
      @click="toggle"
      @keydown="onKeydown"
    >
      <span class="truncate">{{ currentLabel }}</span>
      <svg
        class="shrink-0 transition-transform duration-150"
        :class="{ 'rotate-180': open }"
        width="12"
        height="12"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <path d="M6 9l6 6 6-6" />
      </svg>
    </button>
    <div
      v-if="open"
      class="absolute left-0 z-50 w-full overflow-y-auto rounded-lg border border-border bg-surface py-xs shadow-[0_16px_40px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark"
      :class="flipUp ? 'bottom-full mb-xs' : 'top-full mt-xs'"
      :style="{ maxHeight: `${panelMaxHeight}px` }"
      role="listbox"
    >
      <button
        v-for="option in options"
        :key="option.value"
        type="button"
        class="flex w-full items-center px-[10px] py-[7px] text-left font-medium transition-colors hover:bg-border disabled:cursor-not-allowed disabled:opacity-50 dark:hover:bg-border-dark"
        :class="[
          size === 'sm' ? 'text-caption' : 'text-body',
          colorClass(option.value),
          option.value === modelValue ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark' : '',
        ]"
        :disabled="option.disabled"
        role="option"
        :aria-selected="option.value === modelValue"
        @click="select(option)"
      >
        {{ option.label ?? option.value }}
      </button>
    </div>
  </div>
</template>
