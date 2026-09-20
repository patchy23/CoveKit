<script setup lang="ts">
/** UiConfirmDialog · 项目统一确认弹窗（标题走 UiModal title，保证 Dialog 有可访问名称）。 */
import UiButton from './UiButton.vue'
import UiModal from './UiModal.vue'

withDefaults(
  defineProps<{
    open: boolean
    title: string
    message: string
    confirmLabel?: string
    danger?: boolean
    error?: string
    /** 异步确认进行中：确认钮转圈并禁用，防止重复点击 */
    loading?: boolean
  }>(),
  { confirmLabel: undefined, error: '', loading: false }
)

const emit = defineEmits<{
  (event: 'confirm'): void
  (event: 'close'): void
}>()
</script>

<template>
  <UiModal :open="open" size="sm" :title="title" @close="emit('close')">
    <p
      class="select-text mb-[20px] text-body leading-relaxed text-secondary dark:text-secondary-dark"
    >
      {{ message }}
    </p>
    <p
      v-if="error"
      class="select-text mb-[12px] text-caption text-danger-strong dark:text-danger-dark"
    >
      {{ error }}
    </p>
    <div class="flex justify-end gap-[8px]">
      <UiButton variant="ghost" @click="emit('close')">取消</UiButton>
      <UiButton
        :variant="danger ? 'danger' : 'primary'"
        :loading="loading"
        @click="emit('confirm')"
      >
        {{ confirmLabel ?? '确定' }}
      </UiButton>
    </div>
  </UiModal>
</template>
