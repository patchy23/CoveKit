<script setup lang="ts">
import { ref } from 'vue'

const props = withDefaults(
  defineProps<{
    modelValue?: string | number
    type?: string
    invalid?: boolean
    modelModifiers?: { number?: boolean; trim?: boolean }
  }>(),
  { modelValue: '', type: 'text', invalid: false, modelModifiers: () => ({}) }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: string | number): void }>()
const input = ref<HTMLInputElement | null>(null)

defineExpose({
  focus: () => input.value?.focus(),
  select: () => input.value?.select(),
})

function onInput(event: Event) {
  let value: string | number = (event.target as HTMLInputElement).value
  if (props.modelModifiers.trim) value = value.trim()
  if (props.modelModifiers.number && value !== '') value = Number(value)
  emit('update:modelValue', value)
}
</script>

<template>
  <input
    ref="input"
    :type="type"
    class="field-input"
    :class="{ 'ui-field-invalid': invalid }"
    :value="modelValue"
    :aria-invalid="invalid || undefined"
    @input="onInput"
  />
</template>
