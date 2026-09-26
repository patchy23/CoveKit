import { Text } from '@codemirror/state'
import { afterEach, expect, it, vi } from 'vitest'
import { createDocumentTextReader } from './documentText'

afterEach(() => vi.restoreAllMocks())

it('一次编辑后的回写、保存和 dirty 读取复用同一全文，包含空文档', () => {
  const read = createDocumentTextReader()
  for (const document of [Text.of(['中🙂'.repeat(200000), 'end']), Text.empty]) {
    const stringify = vi.spyOn(document, 'toString')
    const expected = read(document)
    for (let i = 0; i < 100; i++) expect(read(document)).toBe(expected)
    expect(stringify).toHaveBeenCalledTimes(1)
  }
})

it('内容变化立即更新；只保留最近修订，不将全部撤销历史转换成字符串缓存', () => {
  const read = createDocumentTextReader()
  const initial = Text.of(['first', 'last'])
  const updated = initial.replace(0, 5, Text.of(['second']))
  const stringify = vi.spyOn(initial, 'toString')
  expect(read(initial)).toBe('first\nlast')
  expect(read(updated)).toBe('second\nlast')
  expect(read(initial)).toBe('first\nlast')
  expect(stringify).toHaveBeenCalledTimes(2)
})
