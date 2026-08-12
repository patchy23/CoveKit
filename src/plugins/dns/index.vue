<script setup lang="ts">
/**
 * DNS 工具 · 主容器（页签：DNS 查询 / 云解析管理 / 密钥设置）
 */
import { ref } from 'vue'
import QueryPanel from './QueryPanel.vue'
import CloudPanel from './CloudPanel.vue'
import SettingsPanel from './SettingsPanel.vue'
import { UiTabs } from '@/core/ui'

type TabId = 'query' | 'cloud' | 'settings'

const tab = ref<TabId>('query')

/** 页签定义（查询/云解析/设置） */
const tabs = [
  { value: 'query', label: 'DNS 查询' },
  { value: 'cloud', label: '解析管理' },
  { value: 'settings', label: '密钥设置' },
]
</script>

<template>
  <div class="flex h-full min-h-0 w-full flex-col gap-[12px]">
    <!-- 页签栏 -->
    <UiTabs :model-value="tab" :items="tabs" @update:model-value="tab = $event as TabId" />

    <!-- 页签内容 -->
    <div class="min-h-0 flex-1">
      <QueryPanel v-if="tab === 'query'" class="h-full" />
      <CloudPanel v-else-if="tab === 'cloud'" class="h-full" />
      <SettingsPanel v-else class="h-full" />
    </div>
  </div>
</template>
