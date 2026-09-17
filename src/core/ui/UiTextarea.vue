<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
import type { UiSize } from './types'

withDefaults(
  defineProps<{
    modelValue?: string
    invalid?: boolean
    resize?: 'none' | 'vertical'
    size?: UiSize
  }>(),
  { modelValue: '', invalid: false, resize: 'vertical', size: 'md' }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()
</script>

<template>
  <UiScrollArea as-child axis="vertical" managed>
    <textarea
      class="field-textarea"
      :class="[
        `ui-textarea-${size}`,
        { 'ui-field-invalid': invalid },
        resize === 'none' ? 'resize-none' : 'resize-y',
      ]"
      :value="modelValue"
      :aria-invalid="invalid || undefined"
      @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
    />
  </UiScrollArea>
</template>
