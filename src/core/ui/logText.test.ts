import { describe, expect, it } from 'vitest'
import { tailLogText } from './logText'

describe('日志尾部切片', () => {
  it('空行、CRLF、中文和尾随换行与原有显示语义一致', () => {
    const inputs = [
      '',
      '\n',
      '\r\n',
      '\n\n',
      '\na',
      'a\n',
      'a\r\n',
      'a\n\n',
      '中\r\n🙂\n末',
      '\r',
      'a\rb',
    ]
    for (const content of inputs) {
      for (const limit of [1, 2, 3, 100, 300]) {
        const text = content
          .replace(/\r?\n$/, '')
          .split('\n')
          .slice(-limit)
          .join('\n')
        expect(tailLogText(content, limit)).toEqual({
          text,
          lines: text ? text.split('\n').length : 0,
        })
      }
    }
  })
  it('超长行和大量历史行保持完整尾部，不截断正文', () => {
    const last = '中🙂'.repeat(300000)
    const content =
      Array.from({ length: 10000 }, (_, i) => `line-${i}`).join('\n') + '\n' + last + '\n'
    expect(tailLogText(content, 2)).toEqual({ text: 'line-9999\n' + last, lines: 2 })
    expect(tailLogText(last, 300)).toEqual({ text: last, lines: 1 })
  })
})
