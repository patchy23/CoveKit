<script setup lang="ts">
import { UiButton, UiRadioGroup, UiSearchInput } from '@/core/ui'
import type { CredentialKind } from '@/core/ipc/contracts'

const props = defineProps<{
  query: string
  kindFilter: CredentialKind | 'all'
  chips: Array<{ value: CredentialKind | 'all'; label: string }>
}>()
const emit = defineEmits<{
  create: []
  export: []
  import: []
  'update:query': [value: string]
  'update:kindFilter': [value: CredentialKind | 'all']
}>()
function updateKind(value: string) {
  const chip = props.chips.find((item) => item.value === value)
  if (chip) emit('update:kindFilter', chip.value)
}
</script>

<template>
  <div class="mb-[10px] flex items-center gap-[8px]">
    <UiSearchInput
      :model-value="query"
      size="sm"
      placeholder="搜索名称 / 备注 / 摘要…"
      class="w-[220px]"
      @update:model-value="emit('update:query', $event)"
    />
    <div class="flex-1" />
    <UiButton size="sm" variant="secondary" @click="emit('import')">导入</UiButton>
    <UiButton size="sm" variant="secondary" @click="emit('export')">导出</UiButton>
    <UiButton size="sm" variant="primary" @click="emit('create')">+ 新建凭证</UiButton>
  </div>
  <UiRadioGroup
    :model-value="kindFilter"
    :options="chips"
    name="credential-kind"
    aria-label="凭证类型筛选"
    variant="chips"
    size="xs"
    class="mb-[8px]"
    @update:model-value="updateKind"
  />
</template>
