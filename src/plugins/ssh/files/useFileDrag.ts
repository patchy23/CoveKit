/**
 * 双栏文件拖拽（远程⇄本地上传/下载）：pointer 事件自实现
 * Tauri dragDropEnabled 会吞掉应用内 HTML5 拖拽（AGENTS.md 已知坑），必须用 pointerdown/move/up 手写。
 * 文件跨栏传输使用指针阈值；拖拽起点 = 行 pointerdown，跨栏投放才生效（同栏投放丢弃）。
 */
import { onScopeDispose, ref, type Ref } from 'vue'
import type { RemoteFile } from '../contracts'

/** 拖动超过该位移才算拖拽（避免吞掉单击/双击） */
export const DRAG_THRESHOLD = 6

export type DragSide = 'remote' | 'local'

export interface PaneRect {
  left: number
  right: number
  top: number
  bottom: number
}

export interface FileDragState {
  /** 来源栏：remote=远程列表（拖出=下载），local=本地列表（拖出=上传） */
  source: DragSide
  items: RemoteFile[]
  startX: number
  startY: number
  x: number
  y: number
  /** 是否已越过阈值进入拖拽态 */
  active: boolean
}

/** 指针位置 → 目标栏（纯函数；不在两栏区域内返回 null，中列同 null） */
export function resolveDragSide(
  x: number,
  y: number,
  remote: PaneRect,
  local: PaneRect
): DragSide | null {
  const inPane = (r: PaneRect) => x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
  if (inPane(remote)) return 'remote'
  if (inPane(local)) return 'local'
  return null
}

export function useFileDrag(deps: {
  remotePane: Ref<HTMLElement | null>
  localPane: Ref<HTMLElement | null>
  /** 跨栏投放：source→target 的 items */
  onDrop: (source: DragSide, target: DragSide, items: RemoteFile[]) => void
}) {
  const drag = ref<FileDragState | null>(null)
  /** 当前高亮的目标栏 */
  const dragTarget = ref<DragSide | null>(null)

  function paneRect(side: DragSide): PaneRect | null {
    const el = side === 'remote' ? deps.remotePane.value : deps.localPane.value
    if (!el) return null
    const b = el.getBoundingClientRect()
    return { left: b.left, right: b.right, top: b.top, bottom: b.bottom }
  }

  /** 行按下：记录候选（点击/拖拽共用同一入口，拖拽不吞单击） */
  function onRowPointerDown(event: PointerEvent, source: DragSide, file: RemoteFile) {
    if (event.button !== 0) return
    // 拖拽只携带起点行；批量传输使用选中后的显式操作。
    const items = [file]
    drag.value = {
      source,
      items,
      startX: event.clientX,
      startY: event.clientY,
      x: event.clientX,
      y: event.clientY,
      active: false,
    }
    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', onPointerUp, { once: true })
    window.addEventListener('keydown', onKeydown)
  }

  /** 拖拽激活后禁止浏览器文本选择（pointer 拖拽会顺带选中拖动区域文字） */
  let prevUserSelect = ''
  let hadSelectionLock = false
  let selectionLocked = false
  function lockTextSelection() {
    if (selectionLocked) return
    selectionLocked = true
    const body = document.body
    prevUserSelect = body.style.userSelect
    hadSelectionLock = body.classList.contains('ui-drag-select-lock')
    body.classList.add('ui-drag-select-lock')
    body.style.userSelect = 'none'
    const sel = window.getSelection()
    if (sel && !sel.isCollapsed) sel.removeAllRanges()
  }
  function restoreTextSelection() {
    if (!selectionLocked) return
    selectionLocked = false
    document.body.style.userSelect = prevUserSelect
    document.body.classList.toggle('ui-drag-select-lock', hadSelectionLock)
  }

  function onPointerMove(event: PointerEvent) {
    const d = drag.value
    if (!d) return
    if (!d.active) {
      if (Math.hypot(event.clientX - d.startX, event.clientY - d.startY) < DRAG_THRESHOLD) return
      d.active = true
      lockTextSelection()
    }
    d.x = event.clientX
    d.y = event.clientY
    const remote = paneRect('remote')
    const local = paneRect('local')
    const hit = resolveDragSide(
      event.clientX,
      event.clientY,
      remote ?? { left: 0, right: 0, top: 0, bottom: 0 },
      local ?? { left: 0, right: 0, top: 0, bottom: 0 }
    )
    dragTarget.value = hit && hit !== d.source ? hit : null
  }

  function cleanup() {
    drag.value = null
    dragTarget.value = null
    restoreTextSelection()
    window.removeEventListener('pointermove', onPointerMove)
    window.removeEventListener('pointerup', onPointerUp)
    window.removeEventListener('keydown', onKeydown)
  }

  function onPointerUp(event: PointerEvent) {
    const d = drag.value
    cleanup()
    if (!d) return
    // 兜底：合成/极快拖拽可能没有中间 pointermove——up 相对起点位移过阈值即按拖拽处理
    if (!d.active) {
      if (Math.hypot(event.clientX - d.startX, event.clientY - d.startY) < DRAG_THRESHOLD) return
      d.active = true
    }
    d.x = event.clientX
    d.y = event.clientY
    const remote = paneRect('remote')
    const local = paneRect('local')
    if (!remote || !local) return
    const target = resolveDragSide(event.clientX, event.clientY, remote, local)
    if (!target || target === d.source) return
    deps.onDrop(d.source, target, d.items)
  }

  /** Esc 取消拖拽 */
  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') cleanup()
  }

  onScopeDispose(cleanup)
  return { drag, dragTarget, onRowPointerDown }
}
