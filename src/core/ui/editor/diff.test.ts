/**
 * diff.ts 单测：行数统计、CRLF 归一、相同内容判定。
 */
import { describe, expect, it } from 'vitest'
import { diffStats } from './diff'

describe('diffStats', () => {
  it('内容相同时 same 为 true 且无增删', () => {
    expect(diffStats('a\nb\nc', 'a\nb\nc')).toEqual({ added: 0, removed: 0, same: true })
  })

  it('新增一行计入 added', () => {
    const stats = diffStats('a\nb', 'a\nb\nc')
    expect(stats.same).toBe(false)
    expect(stats.added).toBe(1)
    expect(stats.removed).toBe(0)
  })

  it('删除一行计入 removed', () => {
    const stats = diffStats('a\nb\nc', 'a\nc')
    expect(stats.removed).toBe(1)
    expect(stats.added).toBe(0)
  })

  it('改写一行同时体现新增与删除', () => {
    const stats = diffStats('one\ntwo', 'one\nTWO')
    expect(stats.added).toBe(1)
    expect(stats.removed).toBe(1)
  })

  it('CRLF 与 LF 视为一致（避免整篇被判为变更）', () => {
    expect(diffStats('a\r\nb', 'a\nb').same).toBe(true)
  })

  it('空文本与有内容对比', () => {
    const stats = diffStats('', 'a\nb')
    expect(stats.added).toBe(2)
    expect(stats.removed).toBe(0)
    expect(stats.same).toBe(false)
  })

  it('末尾换行不产生额外行差异', () => {
    expect(diffStats('a\nb', 'a\nb\n').same).toBe(true)
  })
})
