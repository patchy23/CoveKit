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
    <button
      v-for="chip in chips"
      :key="chip.value"
      type="button"
      class="rounded-full border px-[10px] py-[2px] text-caption transition-colors"
      :class="
        kindFilter === chip.value
          ? 'border-tertiary-strong bg-tertiary-strong/10 text-tertiary-strong'
          : 'border-border text-secondary hover:bg-border/50 dark:border-border-dark dark:text-secondary-dark'
      "
      @click="emit('update:kindFilter', chip.value)"
    >
      {{ chip.label }}
    </button>
  </div>
</template>
