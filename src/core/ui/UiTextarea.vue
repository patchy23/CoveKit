<script setup lang="ts">
withDefaults(
  defineProps<{
    modelValue?: string
    invalid?: boolean
    resize?: 'none' | 'vertical'
  }>(),
  { modelValue: '', invalid: false, resize: 'vertical' }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>()
</script>

<template>
  <textarea
    class="field-textarea"
    :class="[{ 'ui-field-invalid': invalid }, resize === 'none' ? 'resize-none' : 'resize-y']"
    :value="modelValue"
    :aria-invalid="invalid || undefined"
    @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
  />
</template>
