/**
 * 编辑器校验（lint）：按语言装配 linter，错误以波浪线 + 中文提示呈现
 *
 * - JSON：直接用 `@codemirror/lang-json` 的 `jsonParseLinter()`
 * - XML / SVG / plist：DOMParser 解析，从 parsererror 文本提取行列
 * - SQL：只做括号配对与引号闭合检查（不引 SQL 语法解析器，控制体积与误报）
 * 诊断位置与消息均由纯函数产出，便于单测。
 */
import { jsonParseLinter } from '@codemirror/lang-json'
import { linter, type Diagnostic } from '@codemirror/lint'
import type { Extension } from '@codemirror/state'

/** 行 / 列（1 起始）→ 文档偏移（越界自动收敛） */
export function lineColToOffset(text: string, line: number, col: number): number {
  const lines = text.split('\n')
  const targetLine = Math.min(Math.max(Math.trunc(line) || 1, 1), lines.length)
  let offset = 0
  for (let i = 0; i < targetLine - 1; i += 1) {
    offset += (lines[i]?.length ?? 0) + 1
  }
  const column = Math.min(
    Math.max(Math.trunc(col) || 1, 1),
    (lines[targetLine - 1]?.length ?? 0) + 1
  )
  return Math.min(text.length, offset + column - 1)
}

/** XML 校验：解析失败时给出位置与中文提示（成功返回空数组） */
export function xmlDiagnostics(text: string): Diagnostic[] {
  if (!text.trim()) return []
  // CDATA 会干扰部分解析器，先替换为等长占位符（保持偏移不变）
  const sanitized = text.replace(/<!\[CDATA\[[\s\S]*?\]\]>/g, (match) => 'x'.repeat(match.length))
  const parsed = new DOMParser().parseFromString(sanitized, 'text/xml')
  const errorNode = parsed.querySelector('parsererror')
  if (!errorNode) return []

  const raw = (errorNode.textContent ?? '').trim().replace(/\s+/g, ' ')
  const location = raw.match(/line (\d+)(?:[^\d]*column (\d+))?/i)
  const line = location ? Number(location[1]) : 1
  const column = location?.[2] ? Number(location[2]) : 1
  const from = lineColToOffset(text, line, column)
  return [
    {
      from,
      to: Math.min(text.length, from + 1),
      severity: 'error',
      message: `XML 解析失败：${raw.slice(0, 160)}`,
    },
  ]
}

/** SQL 基础检查：括号配对、引号闭合（跳过字符串字面量与注释） */
export function sqlDiagnostics(text: string): Diagnostic[] {
  const diagnostics: Diagnostic[] = []
  const brackets: { char: string; pos: number }[] = []
  let quote: { char: string; pos: number } | null = null
  let lineComment = false
  let blockComment = false

  for (let i = 0; i < text.length; i += 1) {
    const char = text[i]
    const next = text[i + 1]

    if (lineComment) {
      if (char === '\n') lineComment = false
      continue
    }
    if (blockComment) {
      if (char === '*' && next === '/') {
        blockComment = false
        i += 1
      }
      continue
    }
    if (quote) {
      if (char === quote.char) {
        // 连续两个引号是转义，继续留在字符串内
        if (next === quote.char) {
          i += 1
          continue
        }
        quote = null
      }
      continue
    }
    if (char === '-' && next === '-') {
      lineComment = true
      i += 1
      continue
    }
    if (char === '/' && next === '*') {
      blockComment = true
      i += 1
      continue
    }
    if (char === "'" || char === '"' || char === '`') {
      quote = { char, pos: i }
      continue
    }
    if (char === '(' || char === '[') {
      brackets.push({ char, pos: i })
      continue
    }
    if (char === ')' || char === ']') {
      const open = brackets.pop()
      const expected = char === ')' ? '(' : '['
      if (!open) {
        diagnostics.push({
          from: i,
          to: i + 1,
          severity: 'error',
          message: `括号不匹配：多余的 ${char}`,
        })
      } else if (open.char !== expected) {
        diagnostics.push({
          from: i,
          to: i + 1,
          severity: 'error',
          message: `括号不匹配：此处是 ${char}，应为 ${open.char === '(' ? ')' : ']'}`,
        })
      }
      continue
    }
  }

  if (quote) {
    diagnostics.push({
      from: quote.pos,
      to: Math.min(text.length, quote.pos + 1),
      severity: 'error',
      message: '字符串未闭合：缺少成对的引号',
    })
  }
  for (const item of brackets) {
    diagnostics.push({
      from: item.pos,
      to: Math.min(text.length, item.pos + 1),
      severity: 'error',
      message: `括号未闭合：缺少 ${item.char === '(' ? ')' : ']'}`,
    })
  }
  return diagnostics
}

/** 按语言返回 linter 扩展（空数组表示该语言不校验） */
export function linterForLanguage(languageId: string): Extension[] {
  if (languageId === 'json' || languageId === 'jsonc') {
    return [linter(jsonParseLinter(), { delay: 300 })]
  }
  if (languageId === 'xml' || languageId === 'svg' || languageId === 'plist') {
    return [linter((view) => xmlDiagnostics(view.state.doc.toString()), { delay: 400 })]
  }
  if (languageId === 'sql') {
    return [linter((view) => sqlDiagnostics(view.state.doc.toString()), { delay: 300 })]
  }
  return []
}
