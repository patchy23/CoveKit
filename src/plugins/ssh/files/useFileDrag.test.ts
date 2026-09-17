import { describe, expect, it, vi } from 'vitest'
import { effectScope, ref } from 'vue'
import { resolveDragSide, useFileDrag, type PaneRect } from './useFileDrag'
import type { RemoteFile } from '../contracts'

const REMOTE: PaneRect = { left: 0, right: 800, top: 0, bottom: 600 }
const LOCAL: PaneRect = { left: 1000, right: 1400, top: 0, bottom: 600 }

it.each(['pointerup', 'Escape', 'dispose'])(
  '拖拽选择锁在 %s 后恢复，单击不改变选择规则',
  (exit) => {
    const scope = effectScope()
    const initialStyle = document.body.style.userSelect
    const initialLock = document.body.classList.contains('ui-drag-select-lock')
    const drag = scope.run(() =>
      useFileDrag({
        remotePane: ref(null),
        localPane: ref(null),
        isSelected: () => false,
        rows: () => [],
        onDrop: vi.fn(),
      })
    )!
    const start = () =>
      drag.onRowPointerDown(
        new PointerEvent('pointerdown', { button: 0, clientX: 10, clientY: 10 }),
        'remote',
        { path: '/test' } as RemoteFile
      )
    try {
      document.body.style.userSelect = 'text'
      start()
      window.dispatchEvent(new PointerEvent('pointerup', { clientX: 10, clientY: 10 }))
      expect(document.body.style.userSelect).toBe('text')
      start()
      window.dispatchEvent(new PointerEvent('pointermove', { clientX: 30, clientY: 10 }))
      expect(document.body.classList.contains('ui-drag-select-lock')).toBe(true)
      if (exit === 'dispose') scope.stop()
      else if (exit === 'Escape')
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
      else window.dispatchEvent(new PointerEvent('pointerup', { clientX: 30, clientY: 10 }))
      expect(document.body.classList.contains('ui-drag-select-lock')).toBe(initialLock)
      expect(document.body.style.userSelect).toBe('text')
    } finally {
      scope.stop()
      document.body.style.userSelect = initialStyle
      document.body.classList.toggle('ui-drag-select-lock', initialLock)
    }
  }
)

describe('resolveDragSide 双栏命中判定', () => {
  it('指针在远程栏 → remote', () => {
    expect(resolveDragSide(400, 300, REMOTE, LOCAL)).toBe('remote')
  })
  it('指针在本地栏 → local', () => {
    expect(resolveDragSide(1200, 300, REMOTE, LOCAL)).toBe('local')
  })
  it('中缝（两栏之间）→ null', () => {
    expect(resolveDragSide(900, 300, REMOTE, LOCAL)).toBeNull()
  })
  it('垂直方向越界 → null', () => {
    expect(resolveDragSide(400, 700, REMOTE, LOCAL)).toBeNull()
    expect(resolveDragSide(1200, -10, REMOTE, LOCAL)).toBeNull()
  })
  it('边界点计入（含 left/right 端点）', () => {
    expect(resolveDragSide(0, 0, REMOTE, LOCAL)).toBe('remote')
    expect(resolveDragSide(800, 600, REMOTE, LOCAL)).toBe('remote')
    expect(resolveDragSide(1000, 0, REMOTE, LOCAL)).toBe('local')
    expect(resolveDragSide(1400, 600, REMOTE, LOCAL)).toBe('local')
  })
})
