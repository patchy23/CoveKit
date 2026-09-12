/**
 * SSH 终端搜索 · 纯函数单测（任务书《2026-09-11-扩展任务书》批 1）
 * 覆盖：shouldSearch（空查询门禁）、formatResultCount（三种计数文案）、searchOptions（选项组装）、
 *       isSearchShortcut（Ctrl/Cmd+F 判定）、SEARCH_DECORATIONS（高亮取 DESIGN tokens 字面值）。
 */
import { describe, expect, it } from 'vitest'
import {
  SEARCH_DECORATIONS,
  formatResultCount,
  isSearchShortcut,
  searchOptions,
  shouldSearch,
} from './useTerminalSearch'

/** 构造 attachCustomKeyEventHandler 收到的键盘事件子集 */
function keyEvent(
  type: string,
  key: string,
  modifiers: { ctrlKey?: boolean; metaKey?: boolean } = {}
): Pick<KeyboardEvent, 'type' | 'ctrlKey' | 'metaKey' | 'key'> {
  return { type, key, ctrlKey: modifiers.ctrlKey ?? false, metaKey: modifiers.metaKey ?? false }
}

describe('shouldSearch（空查询门禁）', () => {
  it('空串与纯空白不触发搜索', () => {
    expect(shouldSearch('')).toBe(false)
    expect(shouldSearch('   ')).toBe(false)
    expect(shouldSearch('\t')).toBe(false)
  })

  it('含非空白字符（含首尾空白）触发搜索', () => {
    expect(shouldSearch('a')).toBe(true)
    expect(shouldSearch('  error ')).toBe(true)
  })
})

describe('formatResultCount（计数文案）', () => {
  it('传 query 且查询无效时留空', () => {
    expect(formatResultCount(0, 0, '')).toBe('')
    expect(formatResultCount(0, 17, '   ')).toBe('')
  })

  it('无匹配显示 0/0（含总数未知）', () => {
    expect(formatResultCount(0, 0)).toBe('0/0')
    expect(formatResultCount(-1, 0)).toBe('0/0')
  })

  it('有匹配显示 序号+1/总数', () => {
    expect(formatResultCount(0, 17)).toBe('1/17')
    expect(formatResultCount(16, 17)).toBe('17/17')
  })

  it('超出高亮上限（总数有、当前未定位）显示 -/总数', () => {
    expect(formatResultCount(-1, 17)).toBe('-/17')
  })
})

describe('searchOptions（addon 选项组装）', () => {
  it('区分大小写与正则透传，incremental 与高亮常开', () => {
    expect(searchOptions({ caseSensitive: true, regex: true })).toEqual({
      caseSensitive: true,
      regex: true,
      incremental: true,
      decorations: SEARCH_DECORATIONS,
    })
    expect(searchOptions({ caseSensitive: false, regex: false })).toEqual({
      caseSensitive: false,
      regex: false,
      incremental: true,
      decorations: SEARCH_DECORATIONS,
    })
  })
})

describe('SEARCH_DECORATIONS（高亮取 DESIGN tokens 字面值）', () => {
  it('配色与 tokens 一致：current 用 tertiary-strong，其余用 tertiary 系', () => {
    expect(SEARCH_DECORATIONS).toEqual({
      matchBackground: '#3a2116',
      matchBorder: '#f0562c',
      matchOverviewRuler: '#f0562c',
      activeMatchBackground: '#c2410c',
      activeMatchBorder: '#ff7a4d',
      activeMatchColorOverviewRuler: '#ff7a4d',
    })
  })
})

describe('isSearchShortcut（Ctrl/Cmd+F 拦截判定）', () => {
  it('keydown + Ctrl/Cmd + F（大小写皆可）命中', () => {
    expect(isSearchShortcut(keyEvent('keydown', 'f', { ctrlKey: true }))).toBe(true)
    expect(isSearchShortcut(keyEvent('keydown', 'F', { ctrlKey: true }))).toBe(true)
    expect(isSearchShortcut(keyEvent('keydown', 'f', { metaKey: true }))).toBe(true)
  })

  it('非 F 键、无修饰键、keyup 均不命中', () => {
    expect(isSearchShortcut(keyEvent('keydown', 'c', { ctrlKey: true }))).toBe(false)
    expect(isSearchShortcut(keyEvent('keydown', 'f'))).toBe(false)
    expect(isSearchShortcut(keyEvent('keyup', 'f', { ctrlKey: true }))).toBe(false)
  })
})
