<script setup lang="ts">
/**
 * UiRange · 数值区间滑杆（label 走 UiField；独立使用时务必传 ariaLabel）。
 * 未包在 UiField 里时 inject 到 undefined，label 关联自动跳过。
 */
import { inject } from 'vue'
import { uiFieldContextKey } from './fieldContext'

defineOptions({ inheritAttrs: false })

/** UiField 提供的 label/描述关联 */
const field = inject(uiFieldContextKey, undefined)

withDefaults(
  defineProps<{
    modelValue: number
    min?: number
    max?: number
    step?: number
    disabled?: boolean
    /** 独立使用（无 UiField 包裹）时的可访问名称 */
    ariaLabel?: string
  }>(),
  { min: 0, max: 100, step: 1, disabled: false, ariaLabel: undefined }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: number): void }>()
</script>

<template>
  <input
    v-bind="$attrs"
    :id="($attrs.id as string | undefined) ?? field?.controlId"
    type="range"
    class="ui-range w-full"
    :value="modelValue"
    :min="min"
    :max="max"
    :step="step"
    :disabled="disabled"
    :aria-label="($attrs['aria-label'] as string | undefined) ?? ariaLabel"
    @input="emit('update:modelValue', Number(($event.target as HTMLInputElement).value))"
  />
</template>
