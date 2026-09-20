<script setup lang="ts">
import { computed, provide, useId } from 'vue'
import type { UiSize } from './types'
import { uiFieldContextKey } from './fieldContext'

const props = defineProps<{
  label?: string
  description?: string
  error?: string
  required?: boolean
  size?: UiSize
}>()

const baseId = useId()
/** 控件 id（label 的 for 目标）与描述文本 id（aria-describedby 目标） */
const context = {
  controlId: `ui-field-${baseId}`,
  describedById: computed(() => {
    if (props.error) return `ui-field-${baseId}-error`
    if (props.description) return `ui-field-${baseId}-desc`
    return undefined
  }),
}
provide(uiFieldContextKey, context)
</script>

<template>
  <!-- 根是 div 而非 label：插槽内放 UiCheckbox/UiSwitch（自身根也是 label）时嵌套 label 非法，点击行为会错乱 -->
  <div class="flex min-w-0 flex-col gap-xs">
    <label
      v-if="label"
      :for="context.controlId"
      class="field-label"
      :class="size === 'xs' ? 'text-caption' : size === 'sm' ? 'text-body-sm' : 'text-body'"
    >
      {{ label }}<span v-if="required" class="text-danger-strong"> *</span>
    </label>
    <slot />
    <span
      v-if="error"
      :id="`ui-field-${baseId}-error`"
      role="alert"
      class="select-text text-caption text-danger-strong dark:text-danger-dark"
    >
      {{ error }}
    </span>
    <span
      v-else-if="description"
      :id="`ui-field-${baseId}-desc`"
      class="select-text text-caption text-text-muted dark:text-text-muted-dark"
    >
      {{ description }}
    </span>
  </div>
</template>
