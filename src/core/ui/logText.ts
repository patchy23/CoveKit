/** 从尾部定位已有显示行数范围，避免将完整日志拆成数组再拼接。 */
export function tailLogText(content: string, limit: number): { text: string; lines: number } {
  let end = content.length
  if (content.endsWith('\n')) {
    end--
    if (end > 0 && content[end - 1] === '\r') end--
  }
  if (!end) return { text: '', lines: 0 }
  let start = 0
  let lines = 1
  let position = end - 1
  while (position >= 0) {
    const newline = content.lastIndexOf('\n', position)
    if (newline < 0) break
    if (lines === limit) {
      start = newline + 1
      break
    }
    lines++
    position = newline - 1
  }
  const text = content.slice(start, end)
  return { text, lines: text ? lines : 0 }
}
