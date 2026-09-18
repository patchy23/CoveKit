<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
import UiTooltip from './UiTooltip.vue'
/**
 * UiTabsOverflow · 页签溢出收纳（「···」触发器 + 下拉面板，公共组件）
 * 用法：父组件按宽度把放不下的页签传给 items；选中 emit('select')，逐项可关闭 emit('close')。
 * 点击面板外自动收起。样式与 UiSelect 下拉一致（rounded-lg + 边框 + 阴影）。
 * 键盘契约对齐 ContextMenu：↑↓/Home/End 移动、Enter/空格选中、Esc/Tab 关闭并归还焦点给触发器。
 */
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import UiIcon from './UiIcon.vue'
import UiTabStatusDot from './UiTabStatusDot.vue'
import type { UiTabItem } from './UiTabs.vue'
import { UI_FLOATING_PANEL_CLASS } from './utils'

withDefaults(
  defineProps<{
    /** 收纳进下拉的页签（顺序即展示顺序） */
    items: UiTabItem[]
    /** 当前激活页签 value（用于高亮） */
    modelValue?: string
    /** 触发器提示文案 */
    title?: string
  }>(),
  { modelValue: '', title: '更多页签' }
)

const emit = defineEmits<{
  (event: 'select', value: string): void
  (event: 'close', value: string): void
}>()

/** 下拉面板开关 */
const open = ref(false)
/** 触发器与面板位置（Teleport 到 body，fixed 定位，避免被页签条 overflow-hidden 裁掉） */
const triggerRef = ref<HTMLElement | null>(null)
const panelRef = ref<HTMLElement | null>(null)
const panelPos = ref({ top: 0, left: 0 })
const PANEL_WIDTH = 220

/** 按触发器当前位置重算面板坐标（面板右缘与触发器右缘对齐） */
function updatePanelPos() {
  const rect = triggerRef.value?.getBoundingClientRect()
  if (!rect) return
  panelPos.value = {
    top: rect.bottom + 4,
    left: Math.max(8, rect.right - PANEL_WIDTH),
  }
}

/** 面板内可聚焦的菜单项 */
function menuItems(): HTMLElement[] {
  return Array.from(panelRef.value?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])
}

/** 关闭面板，按需把焦点归还触发器 */
function closePanel(restoreFocus: boolean) {
  open.value = false
  if (restoreFocus) triggerRef.value?.focus({ preventScroll: true })
}

function toggle() {
  open.value = !open.value
  if (open.value) updatePanelPos()
}

function select(value: string) {
  emit('select', value)
  closePanel(true)
}

/** 打开后面板挂好再把焦点移入第一项 */
watch(open, async (isOpen) => {
  if (!isOpen) return
  await nextTick()
  menuItems()[0]?.focus({ preventScroll: true })
})

/** 菜单键盘导航（对齐 ContextMenu） */
function onPanelKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' || event.key === 'Tab') {
    event.preventDefault()
    event.stopPropagation()
    closePanel(true)
    return
  }
  const items = menuItems()
  if (!items.length) return
  const current = items.indexOf(document.activeElement as HTMLElement)
  let index: number
  if (event.key === 'ArrowDown') index = (current + 1) % items.length
  else if (event.key === 'ArrowUp') index = (current - 1 + items.length) % items.length
  else if (event.key === 'Home') index = 0
  else if (event.key === 'End') index = items.length - 1
  else return
  event.preventDefault()
  event.stopPropagation()
  items[index].focus()
  items[index].scrollIntoView?.({ block: 'nearest' })
}

/** 窗口尺寸变化时面板跟随触发器重定位（面板已关则无事发生） */
function onWindowResize() {
  if (open.value) updatePanelPos()
}

/** 面板外点击收起（面板自身 mousedown 阻止冒泡） */
function onDocMouseDown() {
  open.value = false
}

onMounted(() => {
  document.addEventListener('mousedown', onDocMouseDown)
  window.addEventListener('resize', onWindowResize)
})
onUnmounted(() => {
  document.removeEventListener('mousedown', onDocMouseDown)
  window.removeEventListener('resize', onWindowResize)
})
</script>

<template>
  <div class="relative shrink-0">
    <UiTooltip :content="`${title}（${items.length}）`" :disabled="open">
      <button
        ref="triggerRef"
        type="button"
        class="flex h-[28px] items-center gap-[4px] rounded-md px-[8px] text-body-sm font-medium text-secondary transition-colors hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        :class="{ 'bg-border text-primary dark:bg-border-dark dark:text-primary-dark': open }"
        :aria-label="`${title}（${items.length}）`"
        :aria-expanded="open"
        aria-haspopup="menu"
        @mousedown.stop
        @click.stop="toggle"
      >
        <UiIcon name="dots" :size="14" />
      </button>
    </UiTooltip>

    <!-- 面板 Teleport 到 body（fixed 定位）：页签条 overflow-hidden 会裁掉内部绝对定位的下拉 -->
    <Teleport to="body">
      <UiScrollArea v-if="open" as-child axis="vertical">
        <div
          ref="panelRef"
          role="menu"
          :aria-label="title"
          :class="[UI_FLOATING_PANEL_CLASS, 'fixed max-h-[320px] py-[4px]']"
          :style="{
            top: `${panelPos.top}px`,
            left: `${panelPos.left}px`,
            width: `${PANEL_WIDTH}px`,
          }"
          @mousedown.stop
          @keydown="onPanelKeydown"
        >
          <!-- 行容器是 div[role=menuitem] 而非 button：行内还要嵌独立关闭按钮，button 套 button 非法 -->
          <div
            v-for="item in items"
            :key="item.value"
            role="menuitem"
            tabindex="-1"
            class="group flex cursor-pointer items-center gap-[8px] px-[10px] py-[7px] text-body-sm outline-none transition-colors focus-visible:bg-border dark:focus-visible:bg-border-dark"
            :class="
              item.value === modelValue
                ? 'bg-tertiary-soft font-medium text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
                : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
            "
            @click="select(item.value)"
            @keydown.enter.self="select(item.value)"
            @keydown.space.self.prevent="select(item.value)"
          >
            <UiTabStatusDot v-if="item.status" :status="item.status" :title="item.statusTitle" />
            <span class="min-w-0 flex-1 truncate">{{ item.label }}</span>
            <span
              v-if="item.badge !== undefined"
              class="rounded-full bg-border px-[6px] text-caption dark:bg-border-dark"
              >{{ item.badge }}</span
            >
            <UiTooltip v-if="item.closable" :content="`关闭${item.label}`">
              <button
                type="button"
                class="grid h-[16px] w-[16px] shrink-0 place-items-center rounded-[3px] text-text-muted opacity-0 transition-opacity hover:bg-border hover:text-tertiary-strong focus-visible:opacity-100 group-hover:opacity-100 dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-tertiary-dark"
                :aria-label="`关闭${item.label}`"
                @click.stop="emit('close', item.value)"
              >
                <UiIcon name="x" :size="10" />
              </button>
            </UiTooltip>
          </div>
        </div>
      </UiScrollArea>
    </Teleport>
  </div>
</template>
