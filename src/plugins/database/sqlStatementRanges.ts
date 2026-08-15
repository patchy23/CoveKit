/**
 * SQL 语句范围工具（纯函数，无状态）
 * 语义对齐 dbx：按分号分割语句（忽略字符串/注释内的分号），
 * 提供「光标所在完整语句」与「全量语句列表」供执行范围决策。
 */

/** 语句范围：文档偏移 + 原文 */
export interface SqlTextRange {
  from: number
  to: number
  sql: string
}

/** 引号/注释状态机：跳过字符串、单行注释（-- / #）、块注释内的分号 */
export function splitSqlStatements(text: string): SqlTextRange[] {
  const ranges: SqlTextRange[] = []
  let start = 0
  let i = 0
  const n = text.length
  while (i < n) {
    const ch = text[i]
    // 单行注释：-- 或 #（MySQL 风格）
    if ((ch === '-' && text[i + 1] === '-') || ch === '#') {
      while (i < n && text[i] !== '\n') i++
      continue
    }
    // 块注释
    if (ch === '/' && text[i + 1] === '*') {
      i += 2
      while (i < n && !(text[i] === '*' && text[i + 1] === '/')) i++
      i = Math.min(n, i + 2)
      continue
    }
    // 字符串字面量（单引号；双引号视为标识符引用，同样跳过）
    if (ch === "'" || ch === '"' || ch === '`') {
      const quote = ch
      i++
      while (i < n) {
        if (text[i] === '\\') {
          i += 2
          continue
        }
        if (text[i] === quote) {
          // 相邻引号是转义（'' 表示字面引号）
          if (text[i + 1] === quote) {
            i += 2
            continue
          }
          i++
          break
        }
        i++
      }
      continue
    }
    // 语句分隔符
    if (ch === ';') {
      if (text.slice(start, i).trim()) {
        ranges.push({ from: start, to: i + 1, sql: text.slice(start, i + 1) })
      }
      i++
      start = i
      continue
    }
    i++
  }
  // 尾部无分号语句
  if (start < n && text.slice(start).trim()) {
    ranges.push({ from: start, to: n, sql: text.slice(start) })
  }
  return ranges
}

/** 光标所在语句范围（光标在语句内空白/结尾处也算；无语句返回 null） */
export function statementRangeAtCursor(text: string, cursorPos: number): SqlTextRange | null {
  const ranges = splitSqlStatements(text)
  if (!ranges.length) return null
  for (const range of ranges) {
    if (cursorPos >= range.from && cursorPos <= range.to) return range
  }
  // 光标在两语句之间（如刚输入分号后的空行）：归属后一条语句
  const last = ranges[ranges.length - 1]
  if (cursorPos > last.to) return last
  return null
}

/** 语句去掉首尾空白与结尾分号后的可执行文本 */
export function statementExecutableSql(range: SqlTextRange): string {
  return range.sql.trim().replace(/;\s*$/, '')
}
