/** 指针拖拽避免与 Tauri 原生文件拖放冲突；事件仅随本次拖动存活。 */
import { onBeforeUnmount, ref, type Ref } from 'vue'
import type { ApiRecord } from './contracts'

export function useApiDrag(
  root: Ref<HTMLElement | null>,
  move: (id: number, group: string) => void,
  moveGroup: (path: string, parent: string) => void
) {
  const drag = ref<{
    id: number
    kind: 'api' | 'group'
    name: string
    group: string
    pointerId: number
    startX: number
    startY: number
    x: number
    y: number
    active: boolean
  } | null>(null)
  const target = ref<string | null>(null)
  let suppressClick = false
  let clickTimer: ReturnType<typeof setTimeout> | undefined
  function hit(x: number, y: number) {
    const element = document
      .elementFromPoint(x, y)
      ?.closest<HTMLElement>('[data-api-group-drop], [data-api-root-drop]')
    if (!element || !root.value?.contains(element)) return null
    const current = drag.value
    if (!current) return null
    if (element.hasAttribute('data-api-root-drop'))
      return current.kind === 'group' && current.group.includes('/') ? '' : null
    const path = element.dataset.apiGroupDrop ?? null
    if (current.kind === 'group') {
      const parent = current.group.split('/').slice(0, -1).join('/')
      return !path ||
        path === parent ||
        path === current.group ||
        path.startsWith(`${current.group}/`)
        ? null
        : path
    }
    return path === current.group ? null : path
  }
  function blockClick() {
    suppressClick = true
    clearTimeout(clickTimer)
    clickTimer = setTimeout(() => {
      suppressClick = false
    }, 0)
  }
  function cleanup() {
    drag.value = null
    target.value = null
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onEnd)
    window.removeEventListener('pointercancel', cancel)
    window.removeEventListener('keydown', onKey)
    window.removeEventListener('blur', cancel)
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
    if (!current || current.pointerId !== event.pointerId) return
    if (
      !current.active &&
      Math.hypot(event.clientX - current.startX, event.clientY - current.startY) < 6
    )
      return
    current.active = true
    current.x = event.clientX
    current.y = event.clientY
    event.preventDefault()
    target.value = hit(event.clientX, event.clientY)
  }
  function onEnd(event: PointerEvent) {
    const current = drag.value
    if (!current || current.pointerId !== event.pointerId) return
    const path = current.active ? hit(event.clientX, event.clientY) : null
    if (current.active) blockClick()
    cleanup()
    if (path !== null) {
      if (current.kind === 'group') moveGroup(current.group, path)
      else move(current.id, path)
    }
  }
  function start(event: PointerEvent, api: ApiRecord) {
    begin(event, { id: api.id, name: api.name, group: api.groupName, kind: 'api' })
  }
  function startGroup(event: PointerEvent, path: string) {
    if (!path) return
    begin(event, { id: 0, name: path.split('/').at(-1) || path, group: path, kind: 'group' })
  }
  function begin(
    event: PointerEvent,
    source: { id: number; name: string; group: string; kind: 'api' | 'group' }
  ) {
    if (event.button !== 0) return
    cleanup()
    drag.value = {
      ...source,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      x: event.clientX,
      y: event.clientY,
      active: false,
    }
    window.addEventListener('pointermove', onMove, { passive: false })
    window.addEventListener('pointerup', onEnd)
    window.addEventListener('pointercancel', cancel)
    window.addEventListener('keydown', onKey)
    window.addEventListener('blur', cancel)
  }
  function captureClick(event: MouseEvent) {
    if (suppressClick) {
      event.preventDefault()
      event.stopImmediatePropagation()
    }
  }
  onBeforeUnmount(() => {
    cleanup()
    clearTimeout(clickTimer)
  })
  return { drag, target, start, startGroup, captureClick, cancel }
}
