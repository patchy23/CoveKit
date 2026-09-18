<script setup lang="ts">
/** 请求专用键值行，保留重复键及单行启停，不扩张公共控件契约。 */
import { UiButton, UiCheckbox, UiIcon, UiIconButton, UiInput } from '@/core/ui'
import type { KvRow } from './useHttp'
import { emptyRow } from './requestDraft'
const rows = defineModel<KvRow[]>({ required: true })
defineProps<{ label?: string }>()
</script>
<template>
  <div class="space-y-[5px]">
    <div
      class="grid grid-cols-[22px_minmax(0,1fr)_minmax(0,1.4fr)_26px] gap-[6px] text-caption text-secondary dark:text-secondary-dark"
    >
      <span /><span>名称</span><span>值</span><span />
    </div>
    <div
      v-for="(row, index) in rows"
      :key="row.id"
      class="grid grid-cols-[22px_minmax(0,1fr)_minmax(0,1.4fr)_26px] items-center gap-[6px]"
    >
      <UiCheckbox
        :model-value="row.enabled !== false"
        size="xs"
        :title="`启用第 ${index + 1} 行`"
        @update:model-value="row.enabled = $event"
      />
      <UiInput
        v-model="row.key"
        size="sm"
        class="font-mono"
        :aria-label="`第 ${index + 1} 行名称`"
        placeholder="名称"
        spellcheck="false"
      />
      <UiInput
        v-model="row.value"
        size="sm"
        class="font-mono"
        :aria-label="`第 ${index + 1} 行值`"
        placeholder="值"
        spellcheck="false"
      />
      <UiIconButton size="xs" :label="`删除第 ${index + 1} 行`" @click="rows.splice(index, 1)"
        ><UiIcon name="x" :size="12"
      /></UiIconButton>
    </div>
    <UiButton size="xs" variant="ghost" @click="rows.push(emptyRow())"
      ><UiIcon name="plus" :size="12" />{{ label || '添加一行' }}</UiButton
    >
  </div>
</template>
