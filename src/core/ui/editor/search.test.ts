/**
 * search.ts 单测：匹配扫描的位置/长度/上限/非法正则，以及序号换算。
 */
import { describe, expect, it } from 'vitest'
import { escapeRegExp, matchIndexOf, scanMatches, toSearchQuery } from './search'

const base = { caseSensitive: false, regexp: false, wholeWord: false }

describe('escapeRegExp', () => {
  it('转义全部正则元字符', () => {
    expect(escapeRegExp('a.b*c')).toBe('a\\.b\\*c')
    expect(escapeRegExp('(x)[y]{z}')).toBe('\\(x\\)\\[y\\]\\{z\\}')
  })
})

describe('scanMatches', () => {
  it('字面量模式统计全部匹配并给出长度', () => {
    const scan = scanMatches('foo bar Foo foobar', 'foo', base)
    expect(scan.positions).toEqual([0, 8, 12])
    expect(scan.lengths).toEqual([3, 3, 3])
    expect(scan.error).toBeUndefined()
  })

  it('区分大小写开关生效', () => {
    expect(scanMatches('Foo foo', 'foo', { ...base, caseSensitive: true }).positions).toEqual([4])
    expect(scanMatches('Foo foo', 'foo', base).positions).toEqual([0, 4])
  })

  it('正则模式可用，且非法正则返回中文错误而不是抛异常', () => {
    const scan = scanMatches('a1 b2 c3', '\\d', { ...base, regexp: true })
    expect(scan.positions).toEqual([1, 4, 7])
    expect(scan.lengths).toEqual([1, 1, 1])

    const broken = scanMatches('abc', '([', { ...base, regexp: true })
    expect(broken.positions).toEqual([])
    expect(broken.error).toContain('正则表达式无效')
  })

  it('零长匹配不会死循环', () => {
    const scan = scanMatches('abc', 'x*', { ...base, regexp: true })
    expect(scan.truncated).toBe(false)
    expect(scan.positions.length).toBeGreaterThan(0)
  })

  it('全词匹配排除词内出现', () => {
    const scan = scanMatches('cat category cat', 'cat', { ...base, wholeWord: true })
    expect(scan.positions).toEqual([0, 13])
  })

  it('空查询返回空结果', () => {
    const scan = scanMatches('abc', '', base)
    expect(scan.positions).toEqual([])
    expect(scan.lengths).toEqual([])
  })

  it('匹配数超过上限时标记 truncated', () => {
    const text = 'a'.repeat(6000)
    const scan = scanMatches(text, 'a', base)
    expect(scan.truncated).toBe(true)
    expect(scan.positions.length).toBe(5000)
  })
})

describe('matchIndexOf', () => {
  it('命中匹配起点返回 1 起始序号，未命中返回 0', () => {
    const scan = scanMatches('foo foo foo', 'foo', base)
    expect(matchIndexOf(scan, 0)).toBe(1)
    expect(matchIndexOf(scan, 4)).toBe(2)
    expect(matchIndexOf(scan, 2)).toBe(0)
  })
})

describe('toSearchQuery', () => {
  it('字面量模式设置 literal，正则模式设置 regexp', () => {
    const literal = toSearchQuery('a.b', '', base)
    expect(literal.literal).toBe(true)
    expect(literal.regexp).toBe(false)
    expect(literal.search).toBe('a.b')

    const regexp = toSearchQuery('a.b', 'x', { ...base, regexp: true })
    expect(regexp.regexp).toBe(true)
    expect(regexp.literal).toBe(false)
    expect(regexp.replace).toBe('x')
  })
})
