/** 指针拖动兼容桌面 WebView；每次拖动独占监听器，取消/失焦/卸载完整清理。 */
import { onBeforeUnmount, ref, watch, type Ref } from 'vue'
import { validMove, type UiCollectionMove, type UiDropGuard, type UiTreeItem } from './types'

export function useCollectionDrag(
  root: Ref<HTMLElement | null>,
  options: {
    items: () => UiTreeItem[]
    enabled: () => boolean
    tree: () => boolean
    guard: () => UiDropGuard | undefined
    move: (move: UiCollectionMove) => void
    expand: (item: UiTreeItem) => void
  }
) {
  const drag = ref<{ id: string; label: string; x: number; y: number; active: boolean } | null>(
    null
  )
  const target = ref<UiCollectionMove | null>(null)
  let pointerId = 0,
    startX = 0,
    startY = 0,
    frame = 0
  let hoverId = '',
    hoverSince = 0
  let suppress = false
  let clickTimer: ReturnType<typeof setTimeout> | undefined
  function allowed(move: UiCollectionMove) {
    return (
      options.enabled() &&
      validMove(options.items(), move, options.tree()) &&
      (options.guard()?.(move) ?? true)
    )
  }
  function hit(x: number, y: number): UiCollectionMove | null {
    const element = document.elementFromPoint(x, y)
    if (!element || !root.value?.contains(element) || !drag.value) return null
    const row = element.closest<HTMLElement>('[data-collection-id]')
    let move: UiCollectionMove
    if (!row) {
      // 只有列表末尾空白接受根层放置，避免拖到横向留白时意外出组。
      const rows = root.value.querySelectorAll<HTMLElement>('[data-collection-id]')
      const last = rows.item(rows.length - 1)
      if (last && y < last.getBoundingClientRect().bottom) return null
      move = { id: drag.value.id, targetId: null, position: 'inside' }
    } else {
      const item = options.items().find((item) => item.id === row.dataset.collectionId)
      if (!item) return null
      const rect = row.getBoundingClientRect()
      const fraction = (y - rect.top) / Math.max(1, rect.height)
      const position =
        options.tree() && item.expandable && fraction >= 0.25 && fraction <= 0.75
          ? 'inside'
          : fraction < 0.5
            ? 'before'
            : 'after'
      move = { id: drag.value.id, targetId: item.id, position }
    }
    return allowed(move) ? move : null
  }
  function refresh() {
    const current = drag.value
    if (!current?.active) return
    const nextTarget = hit(current.x, current.y)
    if (
      target.value?.targetId !== nextTarget?.targetId ||
      target.value?.position !== nextTarget?.position ||
      target.value?.id !== nextTarget?.id
    )
      target.value = nextTarget
    const next = target.value?.position === 'inside' ? (target.value.targetId ?? '') : ''
    if (next !== hoverId) {
      hoverId = next
      hoverSince = performance.now()
    }
    const item = options.items().find((item) => item.id === hoverId)
    if (
      item?.expandable &&
      !item.expanded &&
      !item.loading &&
      performance.now() - hoverSince > 650
    ) {
      hoverSince = performance.now()
      options.expand(item)
    }
  }
  function tick() {
    if (!drag.value?.active || !root.value) return
    const rect = root.value.getBoundingClientRect()
    const { x, y } = drag.value
    if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
      const delta = y < rect.top + 28 ? -7 : y > rect.bottom - 28 ? 7 : 0
      if (delta) root.value.scrollTop += delta
    }
    refresh()
    frame = requestAnimationFrame(tick)
  }
  function cleanup() {
    cancelAnimationFrame(frame)
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onEnd)
    window.removeEventListener('pointercancel', cancel)
    window.removeEventListener('keydown', onKey)
    window.removeEventListener('blur', cancel)
    drag.value = null
    target.value = null
    hoverId = ''
  }
  function blockClick() {
    suppress = true
    clearTimeout(clickTimer)
    clickTimer = setTimeout(() => {
      suppress = false
    }, 0)
  }
  function cancel() {
    if (drag.value?.active) blockClick()
    cleanup()
  }
  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault()
      cancel()
    }
  }
  function onMove(event: PointerEvent) {
    const current = drag.value
    if (!current || pointerId !== event.pointerId) return
    if (!options.enabled()) {
      cancel()
      return
    }
    if (!current.active && Math.hypot(event.clientX - startX, event.clientY - startY) < 6) return
    const wasActive = current.active
    current.active = true
    current.x = event.clientX
    current.y = event.clientY
    event.preventDefault()
    refresh()
    if (!wasActive) frame = requestAnimationFrame(tick)
  }
  function onEnd(event: PointerEvent) {
    if (!drag.value || event.pointerId !== pointerId) return
    const move = drag.value.active ? hit(event.clientX, event.clientY) : null
    if (drag.value.active) blockClick()
    cleanup()
    if (move) options.move(move)
  }
  function start(event: PointerEvent, item: UiTreeItem) {
    if (event.button !== 0 || !options.enabled() || item.disabled || item.draggable === false)
      return
    if (
      event.target instanceof Element &&
      event.target.closest('input,textarea,select,a,[contenteditable="true"],[data-no-drag]')
    )
      return
    const button = event.target instanceof Element ? event.target.closest('button') : null
    if (button && !button.hasAttribute('data-drag-handle')) return
    cleanup()
    pointerId = event.pointerId
    startX = event.clientX
    startY = event.clientY
    drag.value = { id: item.id, label: item.label, x: startX, y: startY, active: false }
    window.addEventListener('pointermove', onMove, { passive: false })
    window.addEventListener('pointerup', onEnd)
    window.addEventListener('pointercancel', cancel)
    window.addEventListener('keydown', onKey)
    window.addEventListener('blur', cancel)
  }
  function captureClick(event: MouseEvent) {
    if (suppress) {
      event.preventDefault()
      event.stopImmediatePropagation()
    }
  }
  watch(options.enabled, (enabled) => {
    if (!enabled) cancel()
  })
  onBeforeUnmount(() => {
    cleanup()
    clearTimeout(clickTimer)
  })
  return { drag, target, start, cancel, captureClick, allowed }
}
