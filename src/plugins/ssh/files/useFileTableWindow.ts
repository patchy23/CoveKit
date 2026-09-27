import { computed, nextTick, onScopeDispose, ref, watch, type Ref } from 'vue'
import { useRowWindow } from '@/core/ui'
import type { RemoteFile } from '../contracts'

/** 表格模型保持完整；拖拽源与原生文本选区的节点在交互结束前保留。 */
export function useFileTableWindow(root: Ref<HTMLElement | null>, files: () => RemoteFile[]) {
  const rowHeight = ref(35)
  const headerHeight = ref(33)
  const windowed = useRowWindow(
    root,
    () => files().length,
    () => rowHeight.value,
    () => headerHeight.value
  )
  const selection = ref<[number, number]>()
  const pointer = ref<string>()
  const version = ref(0)
  let disposed = false
  const start = computed(() => (files().length <= 200 ? 0 : windowed.start.value))
  const end = computed(() => (files().length <= 200 ? files().length : windowed.end.value))
  const rows = computed(() => {
    const all = files()
    const indices = new Set<number>()
    for (let i = start.value; i < end.value; i++) indices.add(i)
    if (selection.value) {
      const [first, last] = selection.value
      for (let i = first; i <= Math.min(last, all.length - 1); i++) indices.add(i)
    }
    if (pointer.value) {
      const index = all.findIndex((file) => file.path === pointer.value)
      if (index >= 0) indices.add(index)
    }
    let previous = -1
    return [...indices]
      .sort((a, b) => a - b)
      .map((index) => {
        const gap = (index - previous - 1) * rowHeight.value
        previous = index
        return { file: all[index]!, index, gap }
      })
  })
  const trailing = computed(
    () => Math.max(0, files().length - (rows.value.at(-1)?.index ?? -1) - 1) * rowHeight.value
  )
  function measure() {
    const row = root.value?.querySelector<HTMLElement>('[data-file-row]')
    const header = root.value?.querySelector('thead')
    if (row?.offsetHeight) rowHeight.value = row.offsetHeight
    if (header?.getBoundingClientRect().height)
      headerHeight.value = header.getBoundingClientRect().height
  }
  function rowAt(node: Node | null) {
    const element = node instanceof Element ? node : node?.parentElement
    const row = element?.closest<HTMLElement>('[data-file-row]')
    return row && root.value?.contains(row) ? row : null
  }
  function selected() {
    const value = document.getSelection()
    if (!value || value.isCollapsed) {
      selection.value = undefined
      return
    }
    const anchor = rowAt(value.anchorNode),
      focus = rowAt(value.focusNode)
    if (!anchor || !focus) {
      selection.value = undefined
      return
    }
    const a = files().findIndex((file) => file.path === anchor.dataset.fileRow)
    const b = files().findIndex((file) => file.path === focus.dataset.fileRow)
    selection.value = a < 0 || b < 0 ? undefined : [Math.min(a, b), Math.max(a, b)]
  }
  function pointerDown(event: PointerEvent) {
    if (event.button === 0) pointer.value = rowAt(event.target as Node)?.dataset.fileRow
  }
  function pointerUp() {
    selected()
    pointer.value = undefined
  }
  function scrolling() {
    // 原生拖动文本选区时，先保留锚点到新视口之间的节点，避免自动滚动回收 Range。
    if (
      !selection.value ||
      !pointer.value ||
      document.body.classList.contains('ui-drag-select-lock')
    )
      return
    selection.value = [
      Math.min(selection.value[0], start.value),
      Math.max(selection.value[1], end.value - 1),
    ]
  }
  watch(
    [files, rowHeight],
    ([all, height], [previous, oldHeight]) => {
      version.value++
      const viewport = root.value
      if (!viewport || !previous.length) return
      const index = Math.min(previous.length - 1, Math.floor(viewport.scrollTop / oldHeight))
      const next = all.findIndex((file) => file.path === previous[index]!.path)
      viewport.scrollTop = Math.max(
        0,
        (next < 0 ? Math.min(index, all.length - 1) : next) * height +
          ((viewport.scrollTop % oldHeight) * height) / oldHeight
      )
      viewport.dispatchEvent(new Event('scroll'))
      selected()
    },
    { flush: 'pre' }
  )
  watch(
    root,
    (element, _old, cleanup) => {
      if (!element) return
      const observer =
        typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(measure)
      observer?.observe(element)
      element.addEventListener('scroll', scrolling, { passive: true })
      cleanup(() => {
        observer?.disconnect()
        element.removeEventListener('scroll', scrolling)
      })
      measure()
    },
    { flush: 'post', immediate: true }
  )
  watch(
    rows,
    () => {
      void nextTick(() => {
        if (!disposed) measure()
      })
    },
    { flush: 'post' }
  )
  async function jump(index: number) {
    const viewport = root.value
    if (!viewport) return
    // 使用逻辑行号，不要求目标节点已挂载。
    viewport.scrollTo({ top: index * rowHeight.value, behavior: 'smooth' })
    const request = version.value
    await nextTick()
    if (!disposed && request === version.value) measure()
  }
  document.addEventListener('selectionchange', selected)
  document.addEventListener('pointerup', pointerUp)
  document.addEventListener('pointercancel', pointerUp)
  document.fonts?.addEventListener('loadingdone', measure)
  onScopeDispose(() => {
    disposed = true
    document.removeEventListener('selectionchange', selected)
    document.removeEventListener('pointerup', pointerUp)
    document.removeEventListener('pointercancel', pointerUp)
    document.fonts?.removeEventListener('loadingdone', measure)
  })
  return { rows, trailing, pointerDown, jump }
}
