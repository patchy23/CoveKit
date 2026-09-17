<script setup lang="ts">
import UiTooltip from './UiTooltip.vue'
import { TabsList, TabsRoot, TabsTrigger } from 'reka-ui'
import type { UiSize } from './types'

export interface UiTabItem {
  value: string
  label: string
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
      <TabsTrigger
        v-for="item in items"
        :key="item.value"
        :value="item.value"
        class="ui-tab"
        :class="[`ui-tab-${size}`, { 'ui-tab-active': item.value === modelValue }]"
        :disabled="item.disabled"
        @contextmenu="emit('contextmenu', item.value, $event)"
      >
        <UiTooltip
          v-if="item.status"
          :content="
            item.statusTitle ??
            (item.status === 'success' ? '已连接' : item.status === 'danger' ? '已断开' : '连接中')
          "
        >
          <span
            class="h-[7px] w-[7px] shrink-0 rounded-full"
            :class="{
              'bg-success-strong dark:bg-success-dark': item.status === 'success',
              'bg-danger-strong dark:bg-danger-dark': item.status === 'danger',
              'bg-text-muted dark:bg-text-muted-dark': item.status === 'neutral',
              'bg-tertiary animate-pulse dark:bg-tertiary-dark': item.status === 'progress',
            }"
            aria-hidden="true"
          />
        </UiTooltip>
        <span class="ui-tab-label">{{ item.label }}</span>
        <span v-if="item.badge !== undefined" class="ui-tab-badge">{{ item.badge }}</span>
        <span
          v-if="item.closable"
          role="button"
          tabindex="0"
          class="ml-[2px] grid h-[16px] w-[16px] place-items-center rounded-[3px] text-caption text-text-muted hover:bg-border hover:text-tertiary-strong dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-tertiary-dark"
          :aria-label="`关闭${item.label}`"
          @mousedown.stop.prevent
          @click.stop="emit('close', item.value)"
          @keydown.enter.stop.prevent="emit('close', item.value)"
          @keydown.space.stop.prevent="emit('close', item.value)"
        >
          ×
        </span>
      </TabsTrigger>
    </TabsList>
  </TabsRoot>
</template>
