<script setup lang="ts">
import { UiButton, UiInput } from '@/core/ui'
import { DB_TYPE_OPTIONS, type V2DbType } from './useDatabaseMeta'

defineProps<{ label: string; env: string; dbType: V2DbType }>()
const emit = defineEmits<{
  'update:label': [value: string]
  'update:env': [value: string]
  'update:dbType': [value: V2DbType]
}>()
</script>

<template>
  <div class="grid grid-cols-2 gap-[8px]">
    <label class="text-caption font-medium text-text-muted dark:text-text-muted-dark">
      连接名称
      <UiInput
        :model-value="label"
        class="mt-[4px]"
        size="sm"
        placeholder="例如：开发 · PG 主库"
        @update:model-value="emit('update:label', String($event))"
      />
    </label>
    <label class="text-caption font-medium text-text-muted dark:text-text-muted-dark">
      环境
      <UiInput
        :model-value="env"
        class="mt-[4px]"
        size="sm"
        placeholder="开发"
        @update:model-value="emit('update:env', String($event))"
      />
    </label>
  </div>

  <div>
    <label
      class="mb-[4px] block text-caption font-medium text-text-muted dark:text-text-muted-dark"
    >
      数据库类型
    </label>
    <div class="grid grid-cols-3 gap-[6px]">
      <UiButton
        v-for="item in DB_TYPE_OPTIONS"
        :key="item.value"
        size="xs"
        :variant="dbType === item.value ? 'primary' : 'secondary'"
        block
        @click="emit('update:dbType', item.value)"
      >
        <span class="inline-flex items-center gap-[6px]">
          <img :src="item.icon" :alt="item.label" class="h-[14px] w-[14px] object-contain" />
          {{ item.label }}
        </span>
      </UiButton>
    </div>
  </div>
</template>
