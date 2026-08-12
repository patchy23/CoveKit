<script setup lang="ts">
import { computed } from 'vue'
import type { UiSize } from './types'

const props = withDefaults(
  defineProps<{
    value: number
    max?: number
    size?: UiSize
    tone?: 'accent' | 'success' | 'warning' | 'danger'
    label?: string
    showValue?: boolean
  }>(),
  { max: 100, size: 'md', tone: 'accent', label: '', showValue: false }
)

const percent = computed(() => Math.min(100, Math.max(0, (props.value / props.max) * 100)))
const barClass = computed(
  () =>
    ({
      accent: 'bg-tertiary-strong dark:bg-tertiary-dark',
      success: 'bg-success-strong dark:bg-success-dark',
      warning: 'bg-warning-strong dark:bg-warning-dark',
      danger: 'bg-danger-strong dark:bg-danger-dark',
    })[props.tone]
)
</script>

<template>
  <div>
    <div
      v-if="label || showValue"
      class="mb-xs flex justify-between text-caption text-secondary dark:text-secondary-dark"
    >
      <span>{{ label }}</span
      ><span v-if="showValue">{{ Math.round(percent) }}%</span>
    </div>
    <div
      class="overflow-hidden rounded-full bg-border dark:bg-border-dark"
      :class="
        size === 'xs' ? 'h-[3px]' : size === 'sm' ? 'h-xs' : size === 'lg' ? 'h-[10px]' : 'h-[6px]'
      "
    >
      <div
        class="h-full rounded-full transition-[width] duration-200"
        :class="barClass"
        :style="{ width: `${percent}%` }"
        role="progressbar"
        :aria-valuenow="value"
        :aria-valuemin="0"
        :aria-valuemax="max"
      />
    </div>
  </div>
</template>
