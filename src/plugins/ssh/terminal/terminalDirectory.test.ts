import { expect, it } from 'vitest'
import { parseTerminalDirectory } from './terminalDirectory'
it('目录报告解码中文和空格，只接受当前主机及绝对文件路径', () => {
  expect(parseTerminalDirectory('file://server/home/a%20b/%E4%B8%AD', 'server')).toBe(
    '/home/a b/中'
  )
  expect(parseTerminalDirectory('file:///srv', 'server')).toBe('/srv')
  for (const report of [
    'file://other/srv',
    'https://server/srv',
    'file:///a%00b',
    'file:///a%ZZ',
    'file:///a#b',
  ])
    expect(parseTerminalDirectory(report, 'server')).toBeUndefined()
})
