<script setup lang="ts">
import { UiButton, UiSearchInput } from '@/core/ui'
import type { CredentialKind } from '@/core/ipc/contracts'

defineProps<{
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
  <div class="mb-[8px] flex flex-wrap gap-[6px]">
    <UiButton
      v-for="chip in chips"
      :key="chip.value"
      size="xs"
      variant="ghost"
      class="rounded-full border"
      :aria-pressed="kindFilter === chip.value"
      :class="
        kindFilter === chip.value
          ? 'border-tertiary-strong bg-tertiary-soft text-tertiary-strong hover:bg-tertiary-soft hover:text-tertiary-strong dark:border-tertiary-dark dark:bg-tertiary-soft-dark dark:text-tertiary-dark dark:hover:bg-tertiary-soft-dark dark:hover:text-tertiary-dark'
          : 'border-border dark:border-border-dark'
      "
      @click="emit('update:kindFilter', chip.value)"
    >
      {{ chip.label }}
    </UiButton>
  </div>
</template>
