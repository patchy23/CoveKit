<script setup lang="ts">
/**
 * UiDropdownMenu · 触发器式下拉菜单（按钮 → 面板，与坐标式 ContextMenu 互补）
 * 交互原语走 reka DropdownMenu（锚定、焦点管理、Esc/方向键全套）；
 * 项渲染默认「勾选点 + 文案」，需要复杂行（如页签溢出）时用 #item 插槽自绘。
 */
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger,
} from 'reka-ui'
import UiIcon from './UiIcon.vue'
import type { UiSize } from './types'
import { UI_FLOATING_PANEL_CLASS, uiOptionSizeClass } from './utils'

export interface UiDropdownMenuItem {
  value: string
  label: string
  /** 危险操作（删除等），文案红色 */
  danger?: boolean
  disabled?: boolean
}

withDefaults(
  defineProps<{
    items: UiDropdownMenuItem[]
    /** 当前选中值（渲染 ✓ 标记；纯动作菜单不传） */
    modelValue?: string
    size?: UiSize
    /** 面板对齐触发器的边缘（默认起点对齐） */
    align?: 'start' | 'center' | 'end'
    /** 触发器 aria-label（默认三点图标按钮时必填） */
    triggerLabel?: string
  }>(),
  { modelValue: undefined, size: 'md', align: 'start', triggerLabel: '更多' }
)

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'select', value: string): void
}>()

function choose(item: UiDropdownMenuItem): void {
  emit('update:modelValue', item.value)
  emit('select', item.value)
}
</script>

<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger as-child>
      <!-- 默认触发器为「···」图标按钮；也可用 #trigger 插槽换成任意按钮 -->
      <slot name="trigger">
        <button
          type="button"
          class="grid h-[24px] w-[24px] shrink-0 place-items-center rounded-[6px] text-text-muted transition-colors hover:bg-border hover:text-primary focus-visible:outline-2 focus-visible:outline-accent dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          :aria-label="triggerLabel"
        >
          <UiIcon name="dots" :size="16" />
        </button>
      </slot>
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent
        :align="align"
        :side-offset="4"
        :class="[UI_FLOATING_PANEL_CLASS, 'min-w-[140px] py-[4px]']"
      >
        <DropdownMenuItem
          v-for="item in items"
          :key="item.value"
          :disabled="item.disabled"
          class="group flex w-full cursor-default select-none items-center gap-[8px] px-[10px] font-medium outline-none transition-colors data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50 data-[highlighted]:bg-surface-muted dark:data-[highlighted]:bg-surface-muted-dark"
          :class="[
            uiOptionSizeClass(size),
            item.danger
              ? 'text-danger-strong dark:text-danger-dark'
              : 'text-secondary dark:text-secondary-dark',
          ]"
          @select="choose(item)"
        >
          <slot name="item" :item="item">
            <UiIcon
              name="check"
              :size="12"
              :class="item.value === modelValue ? 'opacity-100' : 'opacity-0'"
            />
            <span class="truncate">{{ item.label }}</span>
          </slot>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
