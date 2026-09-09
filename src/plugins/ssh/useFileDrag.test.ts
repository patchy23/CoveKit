import { describe, expect, it } from 'vitest'
import { resolveDragSide, type PaneRect } from './useFileDrag'

const REMOTE: PaneRect = { left: 0, right: 800, top: 0, bottom: 600 }
const LOCAL: PaneRect = { left: 1000, right: 1400, top: 0, bottom: 600 }

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
