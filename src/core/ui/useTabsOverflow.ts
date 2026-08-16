/**
 * useTabsOverflow · 页签溢出测量（按每个页签文字实际估算宽度 + ResizeObserver）
 * 按容器可用宽度动态计算放得下的页签个数，返回可见/收纳两组 items；激活页签永远保持在可见组尾部。
 */
import { computed, onMounted, onUnmounted, ref, type Ref } from 'vue'
import type { UiTabItem } from './UiTabs.vue'

export interface TabWidthOptions {
  /** 文字以外的固定宽度（padding / 图标 / 关闭钮 / 间距），默认 40 */
  extra?: number
  /** 文字部分宽度上限（与页签 max-width 截断对齐），默认 160 */
  maxLabel?: number
}

/** 单个页签宽度估算：中文按 13px、其余 7.5px，加固定宽度；文字部分 capped */
export function estimateTabWidth(label: string, opts: TabWidthOptions = {}): number {
  const { extra = 40, maxLabel = 160 } = opts
  let text = 0
  for (const ch of label) text += ch.charCodeAt(0) > 255 ? 13 : 7.5
  return Math.ceil(Math.min(text, maxLabel) + extra)
}

/**
 * @param container 页签条容器 ref（量它的 clientWidth）
 * @param items     全部页签
 * @param active    当前激活页签 value（保证可见）
 * @param reserved  为非页签元素预留的宽度（如「新建」按钮、触发器自身）
 * @param widthOpts 页签宽度估算参数（与页签实际样式对齐）
 */
export function useTabsOverflow(
  container: Ref<HTMLElement | null>,
  items: Ref<UiTabItem[]>,
  active: Ref<string>,
  reserved = 60,
  widthOpts: TabWidthOptions = {}
) {
  const containerWidth = ref(0)
  let ro: ResizeObserver | null = null

  onMounted(() => {
    containerWidth.value = container.value?.clientWidth ?? 0
    ro = new ResizeObserver((entries) => {
      containerWidth.value = entries[0]?.contentRect.width ?? 0
    })
    if (container.value) ro.observe(container.value)
  })
  onUnmounted(() => ro?.disconnect())

  /** 按可用宽度切分：可见组 + 收纳组（激活页签若被收纳则提到可见组尾部） */
  const split = computed<{ visible: UiTabItem[]; hidden: UiTabItem[] }>(() => {
    const budget = Math.max(0, containerWidth.value - reserved)
    const visible: UiTabItem[] = []
    const hidden: UiTabItem[] = []
    let used = 0
    for (const item of items.value) {
      const w = estimateTabWidth(item.label, widthOpts)
      if (used + w <= budget || visible.length === 0) {
        visible.push(item)
        used += w
      } else {
        hidden.push(item)
      }
    }
    // 激活页签必须可见：与可见组末尾交换
    const activeIdx = hidden.findIndex((t) => t.value === active.value)
    if (activeIdx >= 0 && visible.length) {
      const activeItem = hidden.splice(activeIdx, 1)[0]
      const last = visible.pop()!
      hidden.unshift(last)
      visible.push(activeItem)
    }
    return { visible, hidden }
  })

  return {
    visibleItems: computed(() => split.value.visible),
    hiddenItems: computed(() => split.value.hidden),
  }
}
