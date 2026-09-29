import { describe, expect, it, vi } from 'vitest'
import {
  HEADER_HEIGHT,
  ROW_HEIGHT,
  ROW_NUMBER_WIDTH,
  drawGrid,
  fitGridText,
  gridColumns,
  hitGridCell,
  visibleGridRange,
} from './databaseResultCanvas'

function canvasContext() {
  const drawnText: string[] = []
  const context = {
    save: vi.fn(),
    restore: vi.fn(),
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    beginPath: vi.fn(),
    rect: vi.fn(),
    clip: vi.fn(),
    strokeRect: vi.fn(),
    measureText: vi.fn((text: string) => ({ width: text.length * 5 }) as TextMetrics),
    fillText: vi.fn((text: string) => drawnText.push(text)),
    set font(_value: string) {},
    set textBaseline(_value: CanvasTextBaseline) {},
    set textAlign(_value: CanvasTextAlign) {},
    set fillStyle(_value: string) {},
    set strokeStyle(_value: string) {},
  }
  return { context: context as unknown as CanvasRenderingContext2D, drawnText }
}

describe('数据库结果画布几何与可视区', () => {
  it('计算列起点，命中固定行号列和滚动后的单元格', () => {
    const widths = [100, 80]
    expect(gridColumns(widths)).toEqual({ offsets: [44, 144, 224], totalWidth: 224 })
    expect(ROW_NUMBER_WIDTH).toBe(44)
    expect(HEADER_HEIGHT).toBe(40)
    expect(ROW_HEIGHT).toBe(25)
    expect(hitGridCell(20, 45, 0, 0, widths, 10)).toEqual({ row: 0, column: -1 })
    expect(hitGridCell(50, 24, 0, 0, widths, 10)).toBeNull()
    expect(hitGridCell(50, 39, 0, 0, widths, 10)).toBeNull()
    expect(hitGridCell(50, 40, 0, 0, widths, 10)).toEqual({ row: 0, column: 0 })
    expect(hitGridCell(45, 50, 20, 25, widths, 10)).toEqual({ row: 1, column: 0 })
    expect(hitGridCell(300, 45, 0, 0, widths, 10)).toBeNull()
  })

  it('只绘制与视口相交的行列，并将画布文本限制在500字符', () => {
    const { context, drawnText } = canvasContext()
    const text = vi.fn((row: number, column: number) => `cell-${row}-${column}`)
    const rowLabel = vi.fn((row: number) => String(row + 1))
    const dirty = vi.fn(() => false)
    drawGrid(context, {
      width: 100,
      height: 50,
      scrollLeft: 0,
      scrollTop: 10,
      widths: [100, 1000],
      rowCount: 10,
      text,
      rowLabel,
      dirty,
      selected: { row: 1, column: 0 },
      theme: {
        background: '#fff',
        foreground: '#222',
        muted: '#777',
        border: '#ddd',
        selected: '#fee',
        dirty: '#ffd',
        font: '13px monospace',
      },
    })

    expect(visibleGridRange(10, 50, 10)).toEqual({ start: 0, end: 3 })
    expect(rowLabel).toHaveBeenCalledTimes(3)
    expect(text.mock.calls.map(([, column]) => column)).toEqual([0, 0, 0])
    expect(dirty).toHaveBeenCalledTimes(3)
    expect(drawnText).toContain('cell-0-0')

    const clipped = fitGridText(context, '值'.repeat(600), 10_000)
    expect(clipped).toHaveLength(500)
    expect(clipped.endsWith('…')).toBe(true)
  })
})
