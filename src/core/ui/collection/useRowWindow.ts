/** 固定行高的视口窗口；仅裁剪 DOM，完整数据、键盘索引和选择模型仍由宿主持有。 */
import { computed, ref, watch, type Ref } from 'vue'

export function useRowWindow(
  root: Ref<HTMLElement | null>,
  count: () => number,
  rowHeight: () => number,
  offset: () => number = () => 0
) {
  const top = ref(0),
    height = ref(0)
  const page = computed(() =>
    Math.max(1, Math.ceil((height.value || window.innerHeight) / rowHeight()))
  )
  const start = computed(() =>
    Math.min(
      Math.max(0, count() - page.value),
      Math.max(0, Math.floor(Math.max(0, top.value - offset()) / rowHeight()) - page.value)
    )
  )
  const end = computed(() => Math.min(count(), start.value + page.value * 3))
  function measure() {
    if (!root.value) return
    top.value = root.value.scrollTop
    height.value = root.value.clientHeight || height.value
  }
  watch(
    root,
    (element, _previous, cleanup) => {
      if (!element) return
      element.addEventListener('scroll', measure, { passive: true })
      window.addEventListener('resize', measure)
      const observer =
        typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(measure)
      observer?.observe(element)
      measure()
      cleanup(() => {
        element.removeEventListener('scroll', measure)
        window.removeEventListener('resize', measure)
        observer?.disconnect()
      })
    },
    { immediate: true, flush: 'post' }
  )
  watch(
    [count, rowHeight],
    () => {
      const element = root.value
      if (!element) return
      const maximum = Math.max(
        0,
        offset() +
          count() * rowHeight() -
          (height.value || element.clientHeight || window.innerHeight)
      )
      if (element.scrollTop > maximum) element.scrollTop = maximum
      measure()
    },
    { flush: 'post' }
  )
  function ensureVisible(index: number) {
    const element = root.value
    if (!element || index < 0 || index >= count()) return
    const position = offset() + index * rowHeight()
    const visibleHeight = height.value || window.innerHeight
    if (position < element.scrollTop) element.scrollTop = position
    else if (position + rowHeight() > element.scrollTop + visibleHeight)
      element.scrollTop = position + rowHeight() - visibleHeight
    measure()
  }
  return { start, end, ensureVisible }
}
