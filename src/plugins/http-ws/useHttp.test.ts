import { describe, expect, it } from 'vitest'
import {
  formatRelativeTime,
  isValidUrl,
  kvToHeaders,
  kvToQuery,
  looksLikeJson,
  mergeQuery,
  parseHeaders,
} from './useHttp'

describe('useHttp', () => {
  it('headers 解析', () => {
    expect(parseHeaders('A: 1\nB: 2\nno-colon')).toEqual([
      ['A', '1'],
      ['B', '2'],
    ])
  })

  it('URL 校验', () => {
    expect(isValidUrl('https://example.com/api')).toBe(true)
    expect(isValidUrl('ws://localhost:9000')).toBe(true)
    expect(isValidUrl('ftp://x.com')).toBe(false)
    expect(isValidUrl('not a url')).toBe(false)
  })

  it('JSON 启发', () => {
    expect(looksLikeJson('{"a":1}')).toBe(true)
    expect(looksLikeJson('[1,2]')).toBe(true)
    expect(looksLikeJson('<html>')).toBe(false)
  })

  it('kv → headers/query/merge', () => {
    const rows = [
      { id: '1', key: 'Accept', value: 'application/json' },
      { id: '2', key: '', value: 'ignored' },
    ]
    expect(kvToHeaders(rows)).toEqual({ Accept: 'application/json' })
    const qRows = [
      { id: '1', key: 'q', value: 'hello world' },
      { id: '2', key: 'n', value: '1' },
    ]
    expect(kvToQuery(qRows)).toBe('q=hello%20world&n=1')
    expect(mergeQuery('https://a.com/x', 'q=1')).toBe('https://a.com/x?q=1')
    expect(mergeQuery('https://a.com/x?b=2', 'q=1')).toBe('https://a.com/x?b=2&q=1')
  })

  it('相对时间', () => {
    const now = new Date()
    const d = new Date(now.getTime() - 5 * 60000)
    const pad = (n: number) => String(n).padStart(2, '0')
    const local = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
    expect(formatRelativeTime(local)).toContain('分钟前')
  })
})
