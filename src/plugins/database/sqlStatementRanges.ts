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
export function splitSqlStatements(text: string, dialect = ''): SqlTextRange[] {
  const ranges: SqlTextRange[] = []
  // 存储程序整体交给后端方言分析，运行按钮不切开程序体。
  const head = text.replace(/^\s*(?:(?:--[^\n]*\n|\/\*[\s\S]*?\*\/)\s*)*/, '').toUpperCase()
  if (
    dialect === 'oracle' &&
    /^(?:DECLARE\b|BEGIN\b|CREATE\s+(?:OR\s+REPLACE\s+)?(?:PROCEDURE|FUNCTION|TRIGGER|PACKAGE)\b)/.test(
      head
    )
  ) {
    return [{ from: 0, to: text.length, sql: text }]
  }
  let trigger = false
  let blockDepth = 0
  let leadingWords: string[] = []
  let start = 0
  let i = 0
  const n = text.length
  while (i < n) {
    const ch = text[i]
    // 单行注释：-- 或 #（MySQL 风格）
    if (
      (ch === '-' && text[i + 1] === '-') ||
      (ch === '#' && !['postgresql', 'sqlite', 'oracle'].includes(dialect))
    ) {
      while (i < n && text[i] !== '\n') i++
      continue
    }
    // 块注释
    if (ch === '/' && text[i + 1] === '*') {
      i += 2
      let depth = 1
      while (i < n && depth) {
        if (text[i] === '/' && text[i + 1] === '*') {
          depth++
          i += 2
        } else if (text[i] === '*' && text[i + 1] === '/') {
          depth--
          i += 2
        } else i++
      }
      continue
    }
    // PostgreSQL dollar quote 的正文可包含引号与分号。
    if (ch === '$' && !['mysql', 'polardb'].includes(dialect)) {
      const delimiter = text.slice(i).match(/^\$(?:[A-Za-z_][\w]*)?\$/)?.[0]
      if (delimiter) {
        const end = text.indexOf(delimiter, i + delimiter.length)
        i = end < 0 ? n : end + delimiter.length
        continue
      }
    }
    if (/[A-Za-z_]/.test(ch)) {
      const from = i++
      while (i < n && /[\w$]/.test(text[i])) i++
      const word = text.slice(from, i).toUpperCase()
      if (leadingWords.length < 5) leadingWords.push(word)
      trigger ||=
        dialect === 'sqlite' && leadingWords[0] === 'CREATE' && leadingWords.includes('TRIGGER')
      if (trigger && (word === 'BEGIN' || word === 'CASE')) blockDepth++
      if (trigger && word === 'END') blockDepth = Math.max(0, blockDepth - 1)
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
    if (ch === ';' && blockDepth === 0) {
      if (text.slice(start, i).trim()) {
        ranges.push({ from: start, to: i + 1, sql: text.slice(start, i + 1) })
      }
      i++
      start = i
      trigger = false
      leadingWords = []
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

/** 语句起始非空白字符的文档偏移（gutter 标记定位：range.from 可能落在上一条语句的行尾换行处） */
export function statementStartOffset(range: SqlTextRange): number {
  const leading = range.sql.length - range.sql.trimStart().length
  return range.from + leading
}

/** 光标所在语句范围（光标在语句内空白/结尾处也算；无语句返回 null） */
export function statementRangeAtCursor(
  text: string,
  cursorPos: number,
  dialect = ''
): SqlTextRange | null {
  const ranges = splitSqlStatements(text, dialect)
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
