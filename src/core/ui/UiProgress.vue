<script setup lang="ts">
import { computed } from 'vue'
import type { UiSize } from './types'

const props = withDefaults(
  defineProps<{
    value?: number
    max?: number
    size?: UiSize
    tone?: 'accent' | 'success' | 'warning' | 'danger'
    label?: string
    showValue?: boolean
    indeterminate?: boolean
  }>(),
  {
    value: 0,
    max: 100,
    size: 'md',
    tone: 'accent',
    label: '',
    showValue: false,
    indeterminate: false,
  }
)

const safeMax = computed(() => (Number.isFinite(props.max) && props.max > 0 ? props.max : 100))
const safeValue = computed(() =>
  Number.isFinite(props.value) ? Math.min(safeMax.value, Math.max(0, props.value)) : 0
)
const percent = computed(() => (safeValue.value / safeMax.value) * 100)
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
      ><span v-if="showValue && !indeterminate">{{ Math.round(percent) }}%</span>
    </div>
    <div
      class="overflow-hidden rounded-full bg-border dark:bg-border-dark"
      :class="
        size === 'xs' ? 'h-[3px]' : size === 'sm' ? 'h-xs' : size === 'lg' ? 'h-[10px]' : 'h-[6px]'
      "
    >
      <div
        class="h-full rounded-full transition-[width] duration-200"
        :class="[barClass, { 'animate-pulse motion-reduce:animate-none': indeterminate }]"
        :style="{ width: indeterminate ? '100%' : `${percent}%` }"
        role="progressbar"
        :aria-label="label || undefined"
        :aria-valuenow="indeterminate ? undefined : safeValue"
        :aria-valuemin="0"
        :aria-valuemax="safeMax"
      />
    </div>
  </div>
</template>
