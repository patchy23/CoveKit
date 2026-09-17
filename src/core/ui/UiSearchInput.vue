<script setup lang="ts">
import UiIcon from './UiIcon.vue'
import UiInput from './UiInput.vue'
import type { UiSize } from './types'

withDefaults(
  defineProps<{
    modelValue: string
    placeholder?: string
    size?: UiSize
    disabled?: boolean
    clearable?: boolean
  }>(),
  { placeholder: '搜索…', size: 'md', disabled: false, clearable: true }
)
const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()
</script>

<template>
  <div
    class="relative"
    :style="{
      '--ui-search-trailing-width': $slots.actions
        ? clearable && modelValue && !disabled
          ? '58px'
          : '36px'
        : undefined,
    }"
  >
    <UiIcon
      name="search"
      :size="14"
      class="pointer-events-none absolute left-[10px] top-1/2 -translate-y-1/2 text-text-muted"
    />
    <UiInput
      :model-value="modelValue"
      :size="size"
      :placeholder="placeholder"
      :disabled="disabled"
      class="ui-search-control"
      @update:model-value="emit('update:modelValue', String($event))"
    />
    <div class="absolute right-[6px] top-1/2 flex -translate-y-1/2 items-center gap-[4px]">
      <button
        v-if="clearable && modelValue && !disabled"
        type="button"
        class="grid h-[18px] w-[18px] place-items-center rounded-full text-caption text-text-muted hover:bg-border hover:text-primary dark:hover:bg-border-dark dark:hover:text-primary-dark"
        aria-label="清空搜索"
        @click="emit('update:modelValue', '')"
      >
        ×
      </button>
      <!-- 尾部动作应使用 xs 图标按钮；与清空搜索并排，始终预留输入空间。 -->
      <slot name="actions" />
    </div>
  </div>
</template>
