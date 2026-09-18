/**
 * Base64 编解码纯函数测试（UTF-8 安全 / 非法输入 / 大文本分块）
 */
import { describe, expect, it } from 'vitest'
import { decodeBase64, encodeBase64 } from './useBase64'

describe('encodeBase64', () => {
  it('ASCII 与中文（UTF-8）编码', () => {
    expect(encodeBase64('hello').output).toBe('aGVsbG8=')
    expect(encodeBase64('你好，CoveKit').ok).toBe(true)
  })
  it('空输入报错', () => {
    expect(encodeBase64('').ok).toBe(false)
  })
  it('大文本分块编码不爆栈', () => {
    const big = '甲乙丙'.repeat(50000)
    expect(encodeBase64(big).ok).toBe(true)
  })
})

describe('decodeBase64', () => {
  it('编码解码往返（含中文）', () => {
    const text = '你好，CoveKit ✓ 123'
    const encoded = encodeBase64(text)
    expect(encoded.ok).toBe(true)
    const decoded = decodeBase64(encoded.output)
    expect(decoded.ok).toBe(true)
    expect(decoded.output).toBe(text)
  })
  it('容忍空白与换行', () => {
    expect(decodeBase64('aGVs\nbG8= ').output).toBe('hello')
  })
  it('非法字符与空输入报错', () => {
    expect(decodeBase64('!!!').ok).toBe(false)
    expect(decodeBase64('').ok).toBe(false)
  })
  it('非 UTF-8 字节（二进制内容）报错而非乱码', () => {
    expect(decodeBase64('//79').ok).toBe(false)
  })
})
