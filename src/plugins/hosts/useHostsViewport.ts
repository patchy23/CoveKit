import { computed, nextTick, onScopeDispose, ref, watch, type Ref } from 'vue'
import { useRowWindow } from '@/core/ui'
import type { HostsEntry } from './useHosts'

/** 仅管理列表节点；完整条目、文本和编辑结果仍属于 HostsList。 */
export function useHostsViewport(root: Ref<HTMLElement | null>, entries: () => HostsEntry[]) {
  const rowHeight = ref(62)
  const focused = ref<string>()
  const composing = ref<string>()
  const window = useRowWindow(
    root,
    () => entries().length,
    () => rowHeight.value
  )
  const nodes = new Map<string, HTMLElement>()
  let focusVersion = 0
  let disposed = false
  const observer = typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(measure)

  function measure() {
    const row = nodes.values().next().value as HTMLElement | undefined
    if (!row || row.offsetHeight === 0) return
    rowHeight.value = row.offsetHeight + (parseFloat(getComputedStyle(row).marginBottom) || 0)
  }
  function bind(id: string, node: Element | null) {
    const previous = nodes.get(id)
    if (previous === node) return
    if (previous) observer?.unobserve(previous)
    if (node instanceof HTMLElement) {
      nodes.set(id, node)
      observer?.observe(node)
      measure()
    } else nodes.delete(id)
  }
  const rows = computed(() => {
    const all = entries()
    const indices = new Set<number>()
    for (let index = window.start.value; index < window.end.value; index++) indices.add(index)
    for (const id of [focused.value, composing.value]) {
      if (id === undefined) continue
      const index = all.findIndex((entry) => entry.id === id)
      if (index >= 0) indices.add(index)
    }
    let previous = -1
    return [...indices]
      .sort((a, b) => a - b)
      .map((index) => {
        const gap = (index - previous - 1) * rowHeight.value
        previous = index
        return { entry: all[index]!, index, gap }
      })
  })
  const trailing = computed(() => {
    const last = rows.value.at(-1)?.index ?? -1
    return Math.max(0, entries().length - last - 1) * rowHeight.value
  })

  // 刷新、删除和字号变化沿稳定行身份锚定，不把旧像素偏移解释为新行号。
  watch(
    [entries, rowHeight],
    ([all, height], [previous, oldHeight]) => {
      const element = root.value
      if (!element || !previous.length) return
      const index = Math.min(previous.length - 1, Math.floor(element.scrollTop / oldHeight))
      const anchor = previous[index]!.id
      const next = all.findIndex((entry) => entry.id === anchor)
      element.scrollTop = Math.max(
        0,
        (next < 0 ? Math.min(index, all.length - 1) : next) * height +
          ((element.scrollTop % oldHeight) * height) / oldHeight
      )
      element.dispatchEvent(new Event('scroll'))
    },
    { flush: 'pre' }
  )

  function rowOf(target: EventTarget | null) {
    return target instanceof Element ? target.closest<HTMLElement>('[data-host-entry]') : null
  }
  function focusIn(event: FocusEvent) {
    focused.value = rowOf(event.target)?.dataset.hostEntry
  }
  function focusOut(event: FocusEvent) {
    focused.value = rowOf(event.relatedTarget)?.dataset.hostEntry
  }
  function compositionStart(event: CompositionEvent) {
    composing.value = rowOf(event.target)?.dataset.hostEntry
  }
  function compositionEnd() {
    composing.value = undefined
  }

  async function keydown(event: KeyboardEvent) {
    if (event.key !== 'Tab' || event.ctrlKey || event.altKey || event.metaKey || event.isComposing)
      return
    const row = rowOf(event.target)
    if (!row) return
    const controls = [
      ...row.querySelectorAll<HTMLElement>(
        'input:not([disabled]),button:not([disabled]),[tabindex="0"]'
      ),
    ]
    const boundary = event.shiftKey ? controls[0] : controls.at(-1)
    if (boundary !== event.target) return
    const index =
      entries().findIndex((entry) => entry.id === row.dataset.hostEntry) + (event.shiftKey ? -1 : 1)
    const entry = entries()[index]
    if (!entry) return
    event.preventDefault()
    const version = ++focusVersion
    const previousFocus = document.activeElement
    window.ensureVisible(index)
    await nextTick()
    if (disposed || version !== focusVersion || document.activeElement !== previousFocus) return
    const targets = nodes
      .get(entry.id)
      ?.querySelectorAll<HTMLElement>('input:not([disabled]),button:not([disabled]),[tabindex="0"]')
    const target = targets?.[event.shiftKey ? targets.length - 1 : 0]
    target?.focus({ preventScroll: true })
  }
  async function reveal(index: number) {
    await nextTick()
    if (!disposed) window.ensureVisible(index)
  }
  document.fonts?.addEventListener('loadingdone', measure)
  onScopeDispose(() => {
    disposed = true
    focusVersion++
    observer?.disconnect()
    document.fonts?.removeEventListener('loadingdone', measure)
    nodes.clear()
  })
  return {
    rows,
    trailing,
    bind,
    focusIn,
    focusOut,
    compositionStart,
    compositionEnd,
    keydown,
    reveal,
  }
}
