/**
 * useTabsOverflow · 页签溢出测量（按每个页签文字实际估算宽度 + ResizeObserver）
 * 按容器可用宽度动态计算放得下的页签个数，返回可见/收纳两组 items；激活页签永远保持在可见组尾部。
 */
import { computed, onMounted, onUnmounted, ref, watch, watchPostEffect, type Ref } from 'vue'
import type { UiTabItem } from './UiTabs.vue'

export interface TabWidthOptions {
  /** 文字以外的固定宽度（padding / 图标 / 关闭钮 / 间距），默认 40 */
  extra?: number
  /** 文字部分宽度上限（与页签 max-width 截断对齐），默认 160 */
  maxLabel?: number
}

/** 单个页签宽度估算：中文按 14px、其余 8px（略偏宽，宁多收不少收），加固定宽度；文字部分 capped */
export function estimateTabWidth(label: string, opts: TabWidthOptions = {}): number {
  const { extra = 40, maxLabel = 160 } = opts
  let text = 0
  for (const ch of label) text += ch.charCodeAt(0) > 255 ? 14 : 8
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
  /** 实测修正量：估算仍溢出时，由渲染后测量反馈逐步多收的页签数 */
  const trim = ref(0)
  let ro: ResizeObserver | null = null

  onMounted(() => {
    containerWidth.value = container.value?.clientWidth ?? 0
    ro = new ResizeObserver((entries) => {
      containerWidth.value = entries[0]?.contentRect.width ?? 0
    })
    if (container.value) ro.observe(container.value)
  })
  onUnmounted(() => ro?.disconnect())

  // 宽度或页签集合变化时重置修正量（重新走估算，再由实测反馈收敛）
  watch([containerWidth, () => items.value.length], () => (trim.value = 0))

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
    // 实测修正：估算偏小时真实渲染仍溢出，逐步多收（可见组至少留 1 个）
    for (let i = 0; i < trim.value && visible.length > 1; i++) {
      hidden.unshift(visible.pop()!)
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

  // 渲染后实测：容器内容宽于可视宽（scrollWidth > clientWidth）说明估算偏小，多收一个直到正好放下。
  // trim 有上限（items.length - 1），收敛后 trim 不再变化，watchPostEffect 不会死循环。
  watchPostEffect(() => {
    void split.value // 跟踪依赖：页签切分/trim/容器宽度变化后重新测量
    const el = container.value
    if (!el) return
    if (el.scrollWidth > el.clientWidth + 1 && trim.value < Math.max(0, items.value.length - 1)) {
      trim.value++
    }
  })

  return {
    visibleItems: computed(() => split.value.visible),
    hiddenItems: computed(() => split.value.hidden),
  }
}
