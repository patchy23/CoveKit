// @vitest-environment node
import { expect, it } from 'vitest'
import { createRegexEngine, type RegexRequest } from './regexSearch'

const request = (overrides: Partial<RegexRequest> = {}): RegexRequest => ({
  id: 1,
  document: 'foo1 foo2\n中文foo3',
  query: 'foo(\\d)',
  replacement: '$1:$&:$$',
  options: { regexp: true, caseSensitive: true, wholeWord: false },
  wordChars: '',
  action: 'scan',
  selection: { from: 0, to: 0 },
  viewport: { from: 0, to: 15 },
  ...overrides,
})

it('复用文档快照，统计、正反跳转和捕获组替换来自同一游标', () => {
  const execute = createRegexEngine()
  expect(execute(request()).scan.positions).toEqual([0, 5, 12])
  expect(
    execute(request({ document: undefined, action: 'next', selection: { from: 0, to: 4 } }))
      .selection
  ).toEqual({ from: 5, to: 9 })
  expect(execute(request({ document: undefined, action: 'previous' })).selection).toEqual({
    from: 12,
    to: 16,
  })
  expect(execute(request({ document: undefined, action: 'replaceAll' })).changes).toEqual([
    { from: 0, to: 4, insert: '1:foo1:$' },
    { from: 5, to: 9, insert: '2:foo2:$' },
    { from: 12, to: 16, insert: '3:foo3:$' },
  ])
})

it('全词遵循 Unicode 单词边界，零长匹配不会无限循环', () => {
  const execute = createRegexEngine()
  expect(
    execute(
      request({ query: 'foo3', options: { regexp: true, caseSensitive: true, wholeWord: true } })
    ).scan.positions
  ).toEqual([])
  expect(
    execute(request({ document: 'aa\nbb', query: '^', action: 'replaceAll', replacement: '>' }))
      .changes
  ).toEqual([
    { from: 0, to: 0, insert: '>' },
    { from: 3, to: 3, insert: '>' },
  ])
  expect(execute(request({ query: '([' })).scan.error).toContain('正则表达式无效')
})
