<script setup lang="ts">
/**
 * ContextMenu · 通用右键菜单（Teleport 到 body）
 * 用法：父组件监听 @contextmenu.prevent 记录坐标，渲染 <ContextMenu :x :y :items @close>。
 * 点击外部 / 菜单项后自动关闭；菜单项支持分隔线与危险样式（红色）。
 * hover 高亮统一 bg-border（与 Select 下拉一致）。
 * size：md（默认，text-body，SSH 文件等场景）；sm（text-body-sm，树节点等紧凑场景）。
 * 宽度不写死：min-w 保底 + 按内容自适应（max-w 兜底），菜单项一律不换行。
 * 注意：面板必须 pointer-events-auto——reka 模态弹窗会把 body 置 pointer-events:none，
 * Teleport 到 body 的菜单若不加会整体点不动（弹窗内右键菜单失效的根因）。
 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { FocusScope } from 'reka-ui'
import UiScrollArea from './UiScrollArea.vue'

export interface ContextMenuItem {
  /** 菜单项文字 */
  label: string
  /** 危险操作（红色文字 + 红色 hover 底） */
  danger?: boolean
  /** 禁用操作（灰色文字，不触发回调） */
  disabled?: boolean
  /** 分隔线（单独成项，无 label） */
  separator?: boolean
  /** 点击回调（先关闭菜单并归还焦点，再执行动作） */
  onClick?: () => void
}

const props = withDefaults(
  defineProps<{
    /** 菜单位置（视口坐标，组件按实际尺寸收拢在视口内） */
    x: number
    y: number
    /** 菜单项列表 */
    items: ContextMenuItem[]
    /** 尺寸：md 默认 / sm 紧凑（树、列表内嵌场景） */
    size?: 'md' | 'sm'
    label?: string
  }>(),
  { size: 'md', label: '操作菜单' }
)

const emit = defineEmits<{
  (e: 'close'): void
}>()

const panel = ref<HTMLElement | null>(null)
const position = ref({ left: props.x, top: props.y })
const previousFocus = typeof document === 'undefined' ? null : document.activeElement
let observer: ResizeObserver | undefined
let closed = false

function close(restoreFocus = false) {
  if (closed) return
  closed = true
  if (restoreFocus && previousFocus instanceof HTMLElement && previousFocus.isConnected) {
    previousFocus.focus()
  }
  emit('close')
}

function handleClick(item: ContextMenuItem) {
  if (item.disabled || item.separator) return
  close(true)
  item.onClick?.()
}

function enabledItems() {
  return Array.from(
    panel.value?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? []
  )
}

function focusMenu(event: Event) {
  event.preventDefault()
  ;(enabledItems()[0] ?? panel.value)?.focus()
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' || event.key === 'Tab') {
    event.preventDefault()
    event.stopPropagation()
    close(true)
    return
  }
  const items = enabledItems()
  if (!items.length) return
  const current = items.indexOf(document.activeElement as HTMLButtonElement)
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

function updatePosition() {
  const rect = panel.value?.getBoundingClientRect()
  if (!rect) return
  position.value = {
    left: Math.max(8, Math.min(props.x, window.innerWidth - rect.width - 8)),
    top: Math.max(8, Math.min(props.y, window.innerHeight - rect.height - 8)),
  }
}

function onOutside(event: MouseEvent) {
  if (!panel.value?.contains(event.target as Node)) close()
}

watch(
  () => [props.x, props.y, props.items],
  async () => {
    await nextTick()
    updatePosition()
  }
)

/**
 * 面板尺寸类（最小宽度 / 纵向间距随档位）。
 * 宽度取「最小宽度 + 按内容自适应」而不是写死宽度：菜单项文案长短差异很大，
 * 写死宽度会把「在资源管理器中显示」这类偏长的项挤成两行。
 */
const panelClass = computed(() =>
  props.size === 'sm'
    ? 'min-w-[140px] w-max max-w-[320px] py-[3px]'
    : 'min-w-[168px] w-max max-w-[340px] py-[4px]'
)
/** 菜单项尺寸类 */
const itemClass = computed(() =>
  props.size === 'sm' ? 'px-[10px] py-[5px] text-body-sm' : 'px-[12px] py-[7px] text-body'
)
/** 分隔线间距 */
const separatorClass = computed(() => (props.size === 'sm' ? 'my-[3px]' : 'my-[4px]'))

onMounted(() => {
  updatePosition()
  document.addEventListener('mousedown', onOutside)
  window.addEventListener('resize', updatePosition)
  if (typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(updatePosition)
    if (panel.value) observer.observe(panel.value)
  }
})
onUnmounted(() => {
  document.removeEventListener('mousedown', onOutside)
  window.removeEventListener('resize', updatePosition)
  observer?.disconnect()
})
</script>

<template>
  <Teleport to="body">
    <FocusScope as-child @mount-auto-focus="focusMenu" @unmount-auto-focus.prevent>
      <UiScrollArea as-child>
        <div
          ref="panel"
          role="menu"
          :aria-label="label"
          tabindex="-1"
          class="pointer-events-auto fixed z-[220] rounded-md border border-border bg-surface shadow-card dark:border-border-dark dark:bg-surface-dark"
          :class="panelClass"
          :style="{
            left: `${position.left}px`,
            top: `${position.top}px`,
            maxWidth: `min(${size === 'sm' ? 320 : 340}px, calc(100vw - 16px))`,
            maxHeight: 'calc(100vh - 16px)',
          }"
          @keydown="onKeydown"
          @pointerdown.stop
          @mousedown.stop
        >
          <template v-for="(item, i) in items" :key="i">
            <div
              v-if="item.separator"
              role="separator"
              class="border-t border-border dark:border-border-dark"
              :class="separatorClass"
            />
            <button
              v-else
              type="button"
              role="menuitem"
              tabindex="-1"
              class="flex w-full items-center whitespace-nowrap outline-none transition-colors focus-visible:bg-border dark:focus-visible:bg-border-dark"
              :class="[
                itemClass,
                item.disabled
                  ? 'cursor-not-allowed text-text-muted opacity-60 dark:text-text-muted-dark'
                  : item.danger
                    ? 'text-danger-strong hover:bg-danger-soft dark:text-danger-dark dark:hover:bg-danger-soft-dark'
                    : 'text-primary hover:bg-border dark:text-primary-dark dark:hover:bg-border-dark',
              ]"
              :disabled="item.disabled"
              @click="handleClick(item)"
            >
              <span class="truncate">{{ item.label }}</span>
            </button>
          </template>
        </div>
      </UiScrollArea>
    </FocusScope>
  </Teleport>
</template>
