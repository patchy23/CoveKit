<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * 格式转换 · 根组件（JSON / XML / 时间戳 / Base64 四个子页签）
 * v-show 保持各面板状态，切换子页签不丢失输入内容。
 */
import { ref } from 'vue'
import { UiTabs } from '@/core/ui'
import JsonPanel from './JsonPanel.vue'
import XmlPanel from './XmlPanel.vue'
import TimestampPanel from './TimestampPanel.vue'
import Base64Panel from './Base64Panel.vue'

type PanelId = 'json' | 'xml' | 'timestamp' | 'base64'

const active = ref<PanelId>('json')

const tabs = [
  { value: 'json', label: 'JSON' },
  { value: 'xml', label: 'XML' },
  { value: 'timestamp', label: '时间戳' },
  { value: 'base64', label: 'Base64' },
]
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[10px]">
    <UiTabs v-model="active" :items="tabs" variant="pill" class="shrink-0" />

    <!-- JSON/XML/Base64：分栏占满高度；时间戳：流式布局内部滚动 -->
    <div v-show="active === 'json'" class="min-h-0 flex-1"><JsonPanel /></div>
    <div v-show="active === 'xml'" class="min-h-0 flex-1"><XmlPanel /></div>
    <div v-show="active === 'base64'" class="min-h-0 flex-1"><Base64Panel /></div>
    <UiScrollArea as-child axis="vertical">
      <div v-show="active === 'timestamp'" class="min-h-0 flex-1 pr-[4px]">
        <TimestampPanel />
      </div>
    </UiScrollArea>
  </div>
</template>
