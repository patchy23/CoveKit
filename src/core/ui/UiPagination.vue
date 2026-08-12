<script setup lang="ts">
import { computed } from 'vue'
import UiButton from './UiButton.vue'
import type { UiSize } from './types'

const props = withDefaults(
  defineProps<{ modelValue: number; totalPages: number; size?: UiSize; siblingCount?: number }>(),
  { size: 'sm', siblingCount: 1 }
)
const emit = defineEmits<{ (event: 'update:modelValue', value: number): void }>()
const pages = computed(() => {
  const start = Math.max(1, props.modelValue - props.siblingCount)
  const end = Math.min(props.totalPages, props.modelValue + props.siblingCount)
  return Array.from({ length: end - start + 1 }, (_, index) => start + index)
})
</script>

<template>
  <nav class="flex items-center gap-xs" aria-label="分页">
    <UiButton
      variant="ghost"
      :size="size"
      :disabled="modelValue <= 1"
      @click="emit('update:modelValue', modelValue - 1)"
      >上一页</UiButton
    >
    <UiButton v-if="pages[0] > 1" variant="ghost" :size="size" @click="emit('update:modelValue', 1)"
      >1</UiButton
    >
    <span v-if="pages[0] > 2" class="px-xs text-caption text-text-muted">…</span>
    <UiButton
      v-for="page in pages"
      :key="page"
      :variant="page === modelValue ? 'primary' : 'ghost'"
      :size="size"
      class="min-w-[28px]"
      @click="emit('update:modelValue', page)"
      >{{ page }}</UiButton
    >
    <span v-if="pages[pages.length - 1] < totalPages - 1" class="px-xs text-caption text-text-muted"
      >…</span
    >
    <UiButton
      v-if="pages[pages.length - 1] < totalPages"
      variant="ghost"
      :size="size"
      @click="emit('update:modelValue', totalPages)"
      >{{ totalPages }}</UiButton
    >
    <UiButton
      variant="ghost"
      :size="size"
      :disabled="modelValue >= totalPages"
      @click="emit('update:modelValue', modelValue + 1)"
      >下一页</UiButton
    >
  </nav>
</template>
