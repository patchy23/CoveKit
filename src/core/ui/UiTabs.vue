<script setup lang="ts">
import UiTabStatusDot from './UiTabStatusDot.vue'
import UiIcon from './UiIcon.vue'
import UiTooltip from './UiTooltip.vue'
import { TabsList, TabsRoot, TabsTrigger } from 'reka-ui'
import type { UiSize } from './types'

export interface UiTabItem {
  value: string
  label: string
  title?: string
  disabled?: boolean
  badge?: string | number
  closable?: boolean
  status?: 'success' | 'danger' | 'neutral' | 'progress'
  /** 状态点 tooltip（默认按连接语义：已连接/已断开/连接中） */
  statusTitle?: string
}

withDefaults(
  defineProps<{
    modelValue: string
    items: UiTabItem[]
    variant?: 'pill' | 'line'
    size?: UiSize
  }>(),
  { variant: 'pill', size: 'md' }
)

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'close', value: string): void
  (event: 'contextmenu', value: string, mouse: MouseEvent): void
}>()
</script>

<template>
  <TabsRoot
    :model-value="modelValue"
    activation-mode="manual"
    @update:model-value="emit('update:modelValue', String($event))"
  >
    <TabsList class="ui-tabs" :class="`ui-tabs-${variant}`">
      <!-- trigger 用 as="div"：内部要嵌独立关闭按钮，button 套 button 非法 -->
      <TabsTrigger
        v-for="item in items"
        :key="item.value"
        as="div"
        :value="item.value"
        class="ui-tab"
        :class="[`ui-tab-${size}`, { 'ui-tab-active': item.value === modelValue }]"
        :disabled="item.disabled"
        @contextmenu="emit('contextmenu', item.value, $event)"
      >
        <UiTabStatusDot v-if="item.status" :status="item.status" :title="item.statusTitle" />
        <UiTooltip :content="item.title || item.label"
          ><span class="ui-tab-label">{{ item.label }}</span></UiTooltip
        >
        <span v-if="item.badge !== undefined" class="ui-tab-badge">{{ item.badge }}</span>
        <button
          v-if="item.closable"
          type="button"
          class="ml-[2px] grid h-[16px] w-[16px] place-items-center rounded-[3px] text-text-muted hover:bg-border hover:text-tertiary-strong dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-tertiary-dark"
          :aria-label="`关闭${item.label}`"
          @mousedown.stop.prevent
          @click.stop="emit('close', item.value)"
          @keydown.enter.stop.prevent="emit('close', item.value)"
          @keydown.space.stop.prevent="emit('close', item.value)"
        >
          <UiIcon name="x" :size="10" />
        </button>
      </TabsTrigger>
    </TabsList>
  </TabsRoot>
</template>
