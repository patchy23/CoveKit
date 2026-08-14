<script setup lang="ts">
defineProps<{
  modelValue: number
  min?: number
  max?: number
  step?: number
  disabled?: boolean
}>()

const emit = defineEmits<{ (event: 'update:modelValue', value: number): void }>()
</script>

<template>
  <input
    type="range"
    class="ui-range w-full"
    :value="modelValue"
    :min="min"
    :max="max"
    :step="step"
    :disabled="disabled"
    @input="emit('update:modelValue', Number(($event.target as HTMLInputElement).value))"
  />
</template>
