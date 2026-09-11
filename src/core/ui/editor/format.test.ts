/**
 * lint.ts / format.ts 的纯函数单测（不触达 DOM 之外的能力）
 */
import { describe, expect, it } from 'vitest'
import { canFormat, formatDocument } from './format'
import { lineColToOffset, sqlDiagnostics, xmlDiagnostics } from './lint'

describe('canFormat', () => {
  it('支持 JSON / XML 系与 SQL，其余不支持', () => {
    for (const id of ['json', 'jsonc', 'xml', 'svg', 'plist', 'sql']) {
      expect(canFormat(id)).toBe(true)
    }
    for (const id of ['python', 'plaintext', 'yaml', 'markdown']) {
      expect(canFormat(id)).toBe(false)
    }
  })
})

describe('formatDocument', () => {
  it('JSON 成功时返回缩进后的文本', () => {
    const result = formatDocument('{"a":1}', 'json')
    expect(result.ok).toBe(true)
    expect(result.output).toBe('{\n  "a": 1\n}')
  })

  it('JSON 失败时给出中文行列原因且保留原文', () => {
    const result = formatDocument('{"a":}', 'json')
    expect(result.ok).toBe(false)
    expect(result.output).toBe('{"a":}')
    expect(result.error).toContain('JSON 语法错误')
  })

  it('空内容直接拒绝', () => {
    const result = formatDocument('   ', 'json')
    expect(result.ok).toBe(false)
    expect(result.error).toContain('内容为空')
  })

  it('SQL 格式化成功且保持关键字大小写', () => {
    const result = formatDocument('select a,b from t where a=1', 'sql')
    expect(result.ok).toBe(true)
    expect(result.output).toContain('select')
  })

  it('不支持的语言给出明确原因', () => {
    const result = formatDocument('print(1)', 'python')
    expect(result.ok).toBe(false)
    expect(result.error).toBe('当前语言不支持格式化')
  })
})

describe('lineColToOffset', () => {
  it('按行列换算偏移并收敛越界值', () => {
    const text = 'abc\ndefg\nhi'
    expect(lineColToOffset(text, 1, 1)).toBe(0)
    expect(lineColToOffset(text, 2, 1)).toBe(4)
    expect(lineColToOffset(text, 2, 3)).toBe(6)
    expect(lineColToOffset(text, 99, 99)).toBe(text.length)
    expect(lineColToOffset(text, 0, 0)).toBe(0)
  })
})

describe('sqlDiagnostics', () => {
  it('合法 SQL 无诊断', () => {
    expect(sqlDiagnostics("select * from t where name = 'a(b' -- (")).toEqual([])
  })

  it('括号未闭合报错', () => {
    const diagnostics = sqlDiagnostics('select * from t where (a = 1')
    expect(diagnostics).toHaveLength(1)
    expect(diagnostics[0].message).toContain('括号未闭合')
  })

  it('多余括号报错', () => {
    const diagnostics = sqlDiagnostics('select 1)')
    expect(diagnostics[0].message).toContain('多余')
  })

  it('引号未闭合报错', () => {
    const diagnostics = sqlDiagnostics("select 'abc")
    expect(diagnostics[0].message).toContain('字符串未闭合')
  })

  it('块注释内的括号被忽略', () => {
    expect(sqlDiagnostics('select 1 /* ( */')).toEqual([])
  })
})

describe('xmlDiagnostics', () => {
  it('合法 XML 无诊断', () => {
    expect(xmlDiagnostics('<a><b/></a>')).toEqual([])
  })

  it('标签不闭合时给出中文诊断', () => {
    const diagnostics = xmlDiagnostics('<a><b></a>')
    expect(diagnostics).toHaveLength(1)
    expect(diagnostics[0].severity).toBe('error')
    expect(diagnostics[0].message).toContain('XML 解析失败')
  })

  it('空内容不报错', () => {
    expect(xmlDiagnostics('')).toEqual([])
  })
})
