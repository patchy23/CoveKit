/**
 * SQL 格式化（纯函数，无状态）
 * 原则：短 SQL 保持一行（仅归并空白）；长 SQL / 含子查询按子句换行并缩进；
 * 不改关键字大小写、不重排内容，避免格式化后"面目全非"。
 */

/** 词法单元 */
interface Token {
  /** 种类：词 / 字符串 / 行注释 / 块注释 / 标点 / 分号 / 逗号 */
  kind: 'word' | 'string' | 'lineComment' | 'blockComment' | 'punct' | 'semi' | 'comma'
  text: string
}

/** 主子句关键字（换行点）；两词组合优先匹配 */
const CLAUSES: string[] = [
  'GROUP BY',
  'ORDER BY',
  'UNION ALL',
  'LEFT JOIN',
  'RIGHT JOIN',
  'INNER JOIN',
  'FULL JOIN',
  'CROSS JOIN',
  'LEFT OUTER JOIN',
  'RIGHT OUTER JOIN',
  'INSERT INTO',
  'DELETE FROM',
  'SELECT',
  'FROM',
  'WHERE',
  'HAVING',
  'LIMIT',
  'OFFSET',
  'UNION',
  'JOIN',
  'SET',
  'VALUES',
  'UPDATE',
]
/** WHERE 内的条件连接词（多行模式下换行 + 一级缩进） */
const CONDITIONS = new Set(['AND', 'OR'])
/** 单行模式的最大长度（超过则按子句换行） */
const SINGLE_LINE_MAX = 88

/** 分词：识别字符串/注释/标点，其余归为词 */
function tokenize(input: string): Token[] {
  const tokens: Token[] = []
  let i = 0
  const n = input.length
  while (i < n) {
    const ch = input[i]
    // 空白跳过（语义由拼接规则承担）
    if (/\s/.test(ch)) {
      i++
      continue
    }
    // 行注释
    if ((ch === '-' && input[i + 1] === '-') || ch === '#') {
      let j = i
      while (j < n && input[j] !== '\n') j++
      tokens.push({ kind: 'lineComment', text: input.slice(i, j).trimEnd() })
      i = j
      continue
    }
    // 块注释
    if (ch === '/' && input[i + 1] === '*') {
      let j = i + 2
      while (j < n && !(input[j] === '*' && input[j + 1] === '/')) j++
      j = Math.min(n, j + 2)
      tokens.push({ kind: 'blockComment', text: input.slice(i, j) })
      i = j
      continue
    }
    // 字符串 / 引用标识符
    if (ch === "'" || ch === '"' || ch === '`') {
      let j = i + 1
      while (j < n) {
        if (input[j] === '\\') {
          j += 2
          continue
        }
        if (input[j] === ch) {
          if (input[j + 1] === ch) {
            j += 2
            continue
          }
          j++
          break
        }
        j++
      }
      tokens.push({ kind: 'string', text: input.slice(i, j) })
      i = j
      continue
    }
    if (ch === ';') {
      tokens.push({ kind: 'semi', text: ';' })
      i++
      continue
    }
    if (ch === ',') {
      tokens.push({ kind: 'comma', text: ',' })
      i++
      continue
    }
    if ('()'.includes(ch)) {
      tokens.push({ kind: 'punct', text: ch })
      i++
      continue
    }
    // 词（标识符/关键字/数字/运算符连续段）
    let j = i
    while (j < n && !/[\s;,'"()`#]/.test(input[j]) && !(input[j] === '-' && input[j + 1] === '-')) {
      // 运算符字符单独切段，避免与标识符粘连（如 a=1 → a = 1 不做，保持原样即可）
      j++
    }
    tokens.push({ kind: 'word', text: input.slice(i, j) })
    i = j
  }
  return tokens
}

/** 词序列在 pos 处是否命中子句关键字（两词组合优先），返回匹配词数 */
function matchClause(tokens: Token[], pos: number): number {
  const wordAt = (k: number) => (tokens[k]?.kind === 'word' ? tokens[k].text.toUpperCase() : '')
  for (const clause of CLAUSES) {
    const parts = clause.split(' ')
    let ok = true
    for (let p = 0; p < parts.length; p++) {
      if (wordAt(pos + p) !== parts[p]) {
        ok = false
        break
      }
    }
    if (ok) return parts.length
  }
  return 0
}

/** 括号后是否为子查询（( SELECT ...） */
function isSubqueryOpen(tokens: Token[], pos: number): boolean {
  if (tokens[pos]?.text !== '(') return false
  let k = pos + 1
  while (tokens[k] && (tokens[k].kind === 'lineComment' || tokens[k].kind === 'blockComment')) k++
  return tokens[k]?.kind === 'word' && tokens[k].text.toUpperCase() === 'SELECT'
}

/** 单行拼接规则：逗号/右括号/分号前不加空格，左括号后不加空格 */
function joinInline(tokens: Token[]): string {
  let out = ''
  for (const t of tokens) {
    const noSpaceBefore = t.kind === 'comma' || t.kind === 'semi' || t.text === ')'
    const prevNoSpaceAfter = out.endsWith('(')
    if (out && !noSpaceBefore && !prevNoSpaceAfter) out += ' '
    out += t.text
    // 行注释后必须换行（否则注释吞掉后续语句）
    if (t.kind === 'lineComment') out += '\n'
  }
  return out.trim()
}

/**
 * 格式化 SQL：
 * - 短语句（≤88 字符、无子查询、无行注释）→ 单行
 * - 否则按主子句换行；AND/OR 缩进对齐；子查询括号内缩进一级
 */
export function formatSql(input: string): string {
  const tokens = tokenize(input)
  if (!tokens.length) return ''
  const inline = joinInline(tokens)
  const hasSubquery = tokens.some((_, i) => isSubqueryOpen(tokens, i))
  const hasLineComment = tokens.some((t) => t.kind === 'lineComment')
  const hasClause = tokens.some((_, i) => matchClause(tokens, i) > 0)
  // 多语句脚本：语句间强制换行（格式化语义是整理脚本，不是拼成一行）
  const multiStatement = tokens.some((t, i) => t.kind === 'semi' && i < tokens.length - 1)
  // 短语句单行：无子查询、无行注释、未超宽、单语句（无子句的简单语句也归并为一行）
  if (!hasSubquery && !hasLineComment && !multiStatement && inline.length <= SINGLE_LINE_MAX)
    return inline
  if (!hasClause && !hasSubquery && !multiStatement) return inline

  // 多行模式：逐 token 排版
  const lines: string[] = []
  let line = ''
  let indent = 0
  /** 当前行额外缩进一级（AND/OR 条件行） */
  let extraIndent = false
  const flush = () => {
    if (line.trim()) {
      lines.push('  '.repeat(indent) + (extraIndent ? '  ' : '') + line.trim())
      extraIndent = false
    }
    line = ''
  }
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i]
    if (t.kind === 'lineComment') {
      flush()
      lines.push('  '.repeat(indent) + t.text)
      continue
    }
    if (t.kind === 'semi') {
      line += ';'
      flush()
      continue
    }
    if (t.text === '(') {
      const sub = isSubqueryOpen(tokens, i)
      const noSpace = line.endsWith('(') || !line.trim()
      line += (noSpace ? '' : ' ') + '('
      if (sub) {
        // 先按旧缩进落「FROM (」行，再提升缩进（顺序反了会把外层行错误缩进）
        flush()
        indent++
      }
      continue
    }
    if (t.text === ')') {
      // 子查询收尾：先落当前行，再降缩进放右括号
      if (indent > 0 && lines.length && !line.trim()) {
        // 已在子查询新行开头
      }
      flush()
      if (indent > 0) indent--
      line = ')' + (tokens[i + 1]?.kind === 'comma' ? '' : '')
      // 若右括号后是逗号，由逗号分支拼接到同一行
      continue
    }
    if (t.kind === 'comma') {
      line += ','
      continue
    }
    // 子句换行（行首不重复换行）
    const hit = t.kind === 'word' ? matchClause(tokens, i) : 0
    if (hit > 0 && line.trim()) {
      flush()
    }
    // AND/OR：WHERE 多条件时换行并缩进一级
    if (t.kind === 'word' && CONDITIONS.has(t.text.toUpperCase()) && line.trim()) {
      flush()
      extraIndent = true
    }
    const noSpaceBefore = line.endsWith('(') || !line.trim()
    line += (noSpaceBefore ? '' : ' ') + t.text
    // 跳过多词子句的后续词
    if (hit > 1) {
      for (let p = 1; p < hit; p++) line += ' ' + tokens[i + p].text
      i += hit - 1
    }
  }
  flush()
  return lines.join('\n')
}
