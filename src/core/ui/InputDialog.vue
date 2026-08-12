<script setup lang="ts">
/** InputDialog · 基于 BaseModal 的项目统一单行输入弹窗。 */
import { nextTick, ref, watch } from 'vue'
import { UiButton, UiInput, UiModal } from '@/core/ui'

const props = defineProps<{
  open: boolean
  title: string
  label: string
  initialValue?: string
  confirmLabel?: string
}>()

const emit = defineEmits<{
  (event: 'confirm', value: string): void
  (event: 'close'): void
}>()

const value = ref('')
const input = ref<{ focus: () => void; select: () => void } | null>(null)

watch(
  () => props.open,
  async (open) => {
    if (!open) return
    value.value = props.initialValue ?? ''
    await nextTick()
    input.value?.focus()
    input.value?.select()
  },
  { immediate: true }
)

function submit() {
  const trimmed = value.value.trim()
  if (trimmed) emit('confirm', trimmed)
}
</script>

<template>
  <UiModal :open="open" width="min(420px, 92vw)" @close="emit('close')">
    <h3 class="mb-[14px] text-card-title font-medium text-primary dark:text-primary-dark">
      {{ title }}
    </h3>
    <label class="field-label" for="action-dialog-input">{{ label }}</label>
    <UiInput
      id="action-dialog-input"
      ref="input"
      v-model="value"
      class="mb-[20px]"
      spellcheck="false"
      @keyup.enter="submit"
    />
    <div class="flex justify-end gap-[8px]">
      <UiButton variant="ghost" @click="emit('close')">取消</UiButton>
      <UiButton variant="primary" :disabled="!value.trim()" @click="submit">
        {{ confirmLabel ?? '确定' }}
      </UiButton>
    </div>
  </UiModal>
</template>
