<script setup lang="ts">
import { UiButton, UiInput, UiModal, UiRadioGroup } from '@/core/ui'

defineProps<{
  transfer: { mode: 'export' | 'import'; path: string } | null
  password: string
  importMode: string
  valid: boolean
}>()

const emit = defineEmits<{
  close: []
  confirm: []
  'update:password': [value: string]
  'update:importMode': [value: string]
}>()
</script>

<template>
  <UiModal
    :open="transfer !== null"
    :title="transfer?.mode === 'export' ? '导出凭证备份' : '导入凭证备份'"
    size="sm"
    @close="emit('close')"
  >
    <div class="flex flex-col gap-[10px]">
      <p class="text-body-sm text-secondary dark:text-secondary-dark">
        {{
          transfer?.mode === 'export'
            ? '备份文件使用独立密码加密（Argon2id + AES-256-GCM），导入时需输入同一密码。'
            : `从 ${transfer?.path ?? ''} 导入，需输入导出时设置的密码。`
        }}
      </p>
      <label class="field-label flex flex-col gap-[6px]">
        备份密码（至少 4 位）
        <UiInput
          :model-value="password"
          type="password"
          placeholder="备份密码"
          @update:model-value="emit('update:password', String($event))"
        />
      </label>
      <UiRadioGroup
        v-if="transfer?.mode === 'import'"
        :model-value="importMode"
        name="import-mode"
        size="sm"
        :options="[
          { value: 'merge', label: '合并', description: '保留现有凭证，同 ID 跳过' },
          { value: 'overwrite', label: '覆盖', description: '清空现有凭证后整体替换' },
        ]"
        @update:model-value="emit('update:importMode', $event)"
      />
    </div>
    <template #footer>
      <UiButton size="sm" variant="ghost" @click="emit('close')">取消</UiButton>
      <UiButton size="sm" variant="primary" :disabled="!valid" @click="emit('confirm')">
        {{ transfer?.mode === 'export' ? '导出' : '导入' }}
      </UiButton>
    </template>
  </UiModal>
</template>
