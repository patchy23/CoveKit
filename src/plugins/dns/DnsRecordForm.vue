<script setup lang="ts">
import { UiButton, UiInput, UiSelect } from '@/core/ui'
import { CLOUD_RECORD_TYPES, TTL_PRESETS } from './useDns'

defineProps<{
  rr: string
  recordType: string
  value: string
  ttl: number
  saving: boolean
}>()

const emit = defineEmits<{
  cancel: []
  save: []
  'update:rr': [value: string]
  'update:recordType': [value: string]
  'update:value': [value: string]
  'update:ttl': [value: number]
}>()
</script>

<template>
  <div
    class="flex shrink-0 flex-wrap items-end gap-[10px] rounded-md border border-tertiary/40 bg-tertiary-soft/30 p-[10px] dark:border-tertiary-dark/40 dark:bg-tertiary-soft-dark/30"
  >
    <label class="flex flex-col gap-[4px]">
      <span class="field-label text-body-sm">主机记录</span>
      <UiInput
        :model-value="rr"
        size="sm"
        class="w-[130px] font-mono"
        placeholder="@ / www"
        spellcheck="false"
        @update:model-value="emit('update:rr', String($event))"
      />
    </label>
    <label class="flex flex-col gap-[4px]">
      <span class="field-label text-body-sm">类型</span>
      <UiSelect
        :model-value="recordType"
        class="!w-[100px]"
        :options="CLOUD_RECORD_TYPES.map((item) => ({ value: item, label: item }))"
        @update:model-value="emit('update:recordType', $event)"
      />
    </label>
    <label class="flex min-w-[180px] flex-1 flex-col gap-[4px]">
      <span class="field-label text-body-sm">记录值</span>
      <UiInput
        :model-value="value"
        size="sm"
        class="font-mono placeholder:font-sans"
        placeholder="目标 IP / 域名"
        spellcheck="false"
        @update:model-value="emit('update:value', String($event))"
      />
    </label>
    <label class="flex flex-col gap-[4px]">
      <span class="field-label text-body-sm">TTL</span>
      <UiSelect
        :model-value="String(ttl)"
        class="!w-[100px]"
        :options="TTL_PRESETS.map((item) => ({ value: String(item), label: `${item}s` }))"
        @update:model-value="emit('update:ttl', Number($event))"
      />
    </label>
    <div class="flex gap-[8px]">
      <UiButton variant="primary" size="sm" :loading="saving" @click="emit('save')">
        {{ saving ? '保存中…' : '保存' }}
      </UiButton>
      <UiButton variant="ghost" size="sm" @click="emit('cancel')">取消</UiButton>
    </div>
  </div>
</template>
