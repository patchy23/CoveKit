<script setup lang="ts">
import { UiField, UiInput, UiSelect } from '@/core/ui'
import { DB_TYPE_OPTIONS, DB_TYPE_META, type V2DbType } from './useDatabaseMeta'

defineProps<{ label: string; dbType: V2DbType }>()
const emit = defineEmits<{
  'update:label': [value: string]
  'update:dbType': [value: V2DbType]
}>()
</script>

<template>
  <div class="grid grid-cols-[1fr_160px] gap-[8px]">
    <UiField label="连接名称" size="xs">
      <UiInput
        :model-value="label"
        size="xs"
        placeholder="例如：开发数据库"
        @update:model-value="emit('update:label', String($event))"
      />
    </UiField>
    <UiField label="数据库类型" size="xs">
      <div class="flex min-w-0 items-center gap-[4px]">
        <img
          :src="DB_TYPE_META[dbType].icon"
          alt=""
          class="h-[14px] w-[14px] shrink-0 object-contain"
        />
        <UiSelect
          :model-value="dbType"
          :options="DB_TYPE_OPTIONS"
          size="xs"
          class="min-w-0 flex-1"
          title="数据库类型"
          @update:model-value="emit('update:dbType', $event as V2DbType)"
        />
      </div>
    </UiField>
  </div>
</template>
