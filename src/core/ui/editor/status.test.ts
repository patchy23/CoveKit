/**
 * status.ts 单测：降级阈值、字节/数字格式化、状态栏拼装。
 */
import { describe, expect, it } from 'vitest'
import {
  buildStatusText,
  degradeLevelFor,
  formatBytes,
  formatNumber,
  formatStatusLine,
  HUGE_FILE_LIMIT,
  LARGE_FILE_LIMIT,
  type EditorStatusInput,
} from './status'

function input(overrides: Partial<EditorStatusInput> = {}): EditorStatusInput {
  return {
    cursor: { line: 12, column: 5, selected: 0 },
    lines: 1234,
    length: 58000,
    languageLabel: 'JSON',
    tabSize: 2,
    readonly: false,
    degrade: 'none',
    ...overrides,
  }
}

describe('degradeLevelFor', () => {
  it('按 512KB / 5MB 两级阈值判定', () => {
    expect(degradeLevelFor(0)).toBe('none')
    expect(degradeLevelFor(LARGE_FILE_LIMIT)).toBe('none')
    expect(degradeLevelFor(LARGE_FILE_LIMIT + 1)).toBe('large')
    expect(degradeLevelFor(HUGE_FILE_LIMIT)).toBe('large')
    expect(degradeLevelFor(HUGE_FILE_LIMIT + 1)).toBe('huge')
  })
})

describe('formatBytes / formatNumber', () => {
  it('字节按 1024 进制换算', () => {
    expect(formatBytes(512)).toBe('512 B')
    expect(formatBytes(2048)).toBe('2.0 KB')
    expect(formatBytes(1536)).toBe('1.5 KB')
    expect(formatBytes(1048576)).toBe('1.00 MB')
  })

  it('数字带千分位且最多保留一位小数', () => {
    expect(formatNumber(7)).toBe('7')
    expect(formatNumber(1234)).toBe('1,234')
    expect(formatNumber(1234567)).toBe('1,234,567')
    expect(formatNumber(1234.55)).toBe('1,234.6')
  })
})

describe('buildStatusText', () => {
  it('拼装行列、语言、缩进、规模与编码', () => {
    const status = buildStatusText(input())
    expect(status.position).toBe('行 12 : 列 5')
    expect(status.language).toBe('JSON')
    expect(status.indent).toBe('空格: 2')
    expect(status.size).toBe('1,234 行 · 56.6 KB')
    expect(status.encoding).toBe('UTF-8')
    expect(status.degrade).toBeNull()
  })

  it('有选中且非只读时显示选中字符数', () => {
    expect(buildStatusText(input({ cursor: { line: 1, column: 1, selected: 24 } })).selection).toBe(
      '选中 24'
    )
    expect(
      buildStatusText(input({ cursor: { line: 1, column: 1, selected: 24 }, readonly: true }))
        .selection
    ).toBeNull()
    expect(buildStatusText(input()).selection).toBeNull()
  })

  it('降级时给出中文提示', () => {
    expect(buildStatusText(input({ degrade: 'large' })).degrade).toContain('512KB')
    expect(buildStatusText(input({ degrade: 'huge' })).degrade).toContain('只读')
  })
})

describe('formatStatusLine', () => {
  it('用中文间隔符拼接可见项，跳过空项', () => {
    const line = formatStatusLine(buildStatusText(input()))
    expect(line).toBe('行 12 : 列 5 · JSON · 空格: 2 · 1,234 行 · 56.6 KB · UTF-8')
    expect(line).not.toContain('选中')

    const withSelection = formatStatusLine(
      buildStatusText(input({ cursor: { line: 3, column: 2, selected: 8 }, degrade: 'large' }))
    )
    expect(withSelection).toContain('选中 8')
    expect(withSelection).toContain('512KB')
  })
})
