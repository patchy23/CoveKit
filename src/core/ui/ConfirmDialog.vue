<script setup lang="ts">
/** ConfirmDialog · 基于 BaseModal 的项目统一确认弹窗。 */
import { UiButton, UiModal } from '@/core/ui'

defineProps<{
  open: boolean
  title: string
  message: string
  confirmLabel?: string
  danger?: boolean
  error?: string
}>()

const emit = defineEmits<{
  (event: 'confirm'): void
  (event: 'close'): void
}>()
</script>

<template>
  <UiModal :open="open" width="min(420px, 92vw)" @close="emit('close')">
    <h3 class="mb-[8px] text-card-title font-medium text-primary dark:text-primary-dark">
      {{ title }}
    </h3>
    <p class="mb-[20px] text-body leading-relaxed text-secondary dark:text-secondary-dark">
      {{ message }}
    </p>
    <p v-if="error" class="mb-[12px] text-caption text-danger-strong dark:text-danger-dark">
      {{ error }}
    </p>
    <div class="flex justify-end gap-[8px]">
      <UiButton variant="ghost" @click="emit('close')">取消</UiButton>
      <UiButton :variant="danger ? 'danger' : 'primary'" @click="emit('confirm')">
        {{ confirmLabel ?? '确定' }}
      </UiButton>
    </div>
  </UiModal>
</template>
