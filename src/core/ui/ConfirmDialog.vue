<script setup lang="ts">
/** ConfirmDialog · 基于 BaseModal 的项目统一确认弹窗。 */
import BaseModal from '@/features/ui/BaseModal.vue'

defineProps<{
  open: boolean
  title: string
  message: string
  confirmLabel?: string
  danger?: boolean
}>()

const emit = defineEmits<{
  (event: 'confirm'): void
  (event: 'close'): void
}>()
</script>

<template>
  <BaseModal :open="open" width="min(420px, 92vw)" @close="emit('close')">
    <h3 class="mb-[8px] text-card-title font-medium text-primary dark:text-primary-dark">
      {{ title }}
    </h3>
    <p class="mb-[20px] text-body leading-relaxed text-secondary dark:text-secondary-dark">
      {{ message }}
    </p>
    <div class="flex justify-end gap-[8px]">
      <button class="btn-ghost" @click="emit('close')">取消</button>
      <button
        class="btn-primary"
        :class="{ '!bg-danger-strong dark:!bg-danger-dark': danger }"
        @click="emit('confirm')"
      >
        {{ confirmLabel ?? '确定' }}
      </button>
    </div>
  </BaseModal>
</template>
