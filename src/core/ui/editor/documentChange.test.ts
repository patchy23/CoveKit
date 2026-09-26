import { ChangeSet, Text } from '@codemirror/state'
import { expect, it, vi } from 'vitest'
import { documentChange } from './documentChange'

const text = (value: string) => Text.of(value.split('\n'))

it.each([
  ['', ''],
  ['', 'a\nb'],
  ['a\nb', ''],
  ['a', 'aa'],
  ['aa', 'a'],
  ['x\n\ny', 'x\ny'],
  ['x\ny', 'x\n\ny'],
  ['abc\ndef', 'ab\nc\ndef'],
  ['a\nb\nc', 'a\nB\nc'],
  ['\n\n\n', '\n'],
  ['same\n', 'same'],
  ['中😀文\n尾', '中😁文\n尾'],
  ['a\rb', 'a\r\nb'],
])('替换后保留完整内容：%j → %j', (before, after) => {
  const current = text(before)
  const next = text(after)
  const change = documentChange(current, next)
  const result = change ? ChangeSet.of(change, current.length).apply(current) : current
  expect(result.eq(next)).toBe(true)
  expect(change === null).toBe(before === after)
})

it('大文档只提交改变的码元，不生成旧文档全文副本', () => {
  const prefix = '共同的前文😀\n'.repeat(20_000)
  const suffix = '\n共同的后文😀'.repeat(20_000)
  const current = text(`${prefix}old${suffix}`)
  const stringify = vi.spyOn(current, 'toString')
  try {
    const change = documentChange(current, text(`${prefix}new${suffix}`))
    expect(change?.from).toBe(prefix.length)
    expect(change?.to).toBe(prefix.length + 3)
    expect(change?.insert.toString()).toBe('new')
    expect(stringify).not.toHaveBeenCalled()
  } finally {
    stringify.mockRestore()
  }
})

it('多层文档树上的分行、合行和跨 Unicode 删除均能完整重放', () => {
  const before = Array.from({ length: 200 }, (_, i) => `${i} 中文😀`).join('\n')
  const current = text(before)
  for (let i = 0; i < 100; i++) {
    const start = (i * 37) % before.length
    const end = Math.min(before.length, start + ((i * 19) % 45))
    const after = before.slice(0, start) + '\n新😀\n' + before.slice(end)
    const next = text(after)
    const change = documentChange(current, next)
    expect(change).not.toBeNull()
    expect(ChangeSet.of(change!, current.length).apply(current).eq(next)).toBe(true)
  }
})
