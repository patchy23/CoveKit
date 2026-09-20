<script setup lang="ts">
import { UiButton, UiCodeEditor, UiModal } from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import type { DbValue } from './contracts'
defineProps<{ detail: { name: string; value: DbValue } | null }>()
const emit = defineEmits<{ close: [] }>()
const { copyText } = useCopy()
</script>
<template>
  <UiModal
    :open="!!detail"
    :title="detail?.name ?? '单元格'"
    :description="
      detail?.value.kind === 'null'
        ? 'SQL NULL'
        : detail?.value.kind === 'binary'
          ? '二进制 · 十六进制'
          : detail?.value.kind
    "
    size="lg"
    @close="emit('close')"
  >
    <UiCodeEditor
      :model-value="detail?.value.value ?? 'NULL'"
      :language="detail?.value.kind === 'json' ? 'json' : 'text'"
      readonly
      class="h-[300px]"
    />
    <template #footer>
      <UiButton size="xs" variant="secondary" @click="emit('close')">关闭</UiButton>
      <UiButton
        size="xs"
        variant="primary"
        @click="
          copyText(
            detail?.value.kind === 'null' ? '\\N' : (detail?.value.value ?? ''),
            '已复制原值'
          )
        "
        >复制原值</UiButton
      >
    </template>
  </UiModal>
</template>
