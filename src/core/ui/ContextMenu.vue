<script setup lang="ts">
/**
 * ContextMenu · 通用右键菜单（Teleport 到 body）
 * 用法：父组件监听 @contextmenu.prevent 记录坐标，渲染 <ContextMenu :x :y :items @close>。
 * 点击外部 / 菜单项后自动关闭；菜单项支持分隔线与危险样式（红色）。
 * hover 高亮统一 bg-border（与 Select 下拉一致）。
 */
import { onMounted, onUnmounted } from 'vue'

export interface ContextMenuItem {
  /** 菜单项文字 */
  label: string
  /** 危险操作（红色文字 + 红色 hover 底） */
  danger?: boolean
  /** 禁用操作（灰色文字，不触发回调） */
  disabled?: boolean
  /** 分隔线（单独成项，无 label） */
  separator?: boolean
  /** 点击回调（触发后自动关闭菜单） */
  onClick?: () => void
}

defineProps<{
  /** 菜单位置（视口坐标，父组件需自行收拢在视口内） */
  x: number
  y: number
  /** 菜单项列表 */
  items: ContextMenuItem[]
}>()

const emit = defineEmits<{
  (e: 'close'): void
}>()

function close() {
  emit('close')
}

/** 菜单项点击：执行回调后关闭菜单（单语句调用，避免模板多语句表达式） */
function handleClick(item: ContextMenuItem) {
  item.onClick?.()
  close()
}

onMounted(() => document.addEventListener('mousedown', close))
onUnmounted(() => document.removeEventListener('mousedown', close))
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed z-[200] w-[150px] overflow-hidden rounded-md border border-border bg-surface py-[4px] shadow-[0_8px_24px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark"
      :style="{ left: `${x}px`, top: `${y}px` }"
      @mousedown.stop
    >
      <template v-for="(item, i) in items" :key="i">
        <div
          v-if="item.separator"
          class="my-[4px] border-t border-border dark:border-border-dark"
        />
        <button
          v-else
          class="flex w-full items-center px-[12px] py-[7px] text-body transition-colors"
          :disabled="item.disabled"
          :class="
            item.disabled
              ? 'cursor-not-allowed text-text-muted opacity-60 dark:text-text-muted-dark'
              : item.danger
                ? 'text-danger-strong hover:bg-danger-soft dark:text-danger-dark dark:hover:bg-danger-soft-dark'
                : 'text-primary hover:bg-border dark:text-primary-dark dark:hover:bg-border-dark'
          "
          @click="handleClick(item)"
        >
          {{ item.label }}
        </button>
      </template>
    </div>
  </Teleport>
</template>
