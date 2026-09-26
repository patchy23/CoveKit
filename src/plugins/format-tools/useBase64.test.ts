/**
 * Base64 编解码纯函数测试（UTF-8 安全 / 非法输入 / 大文本分块）
 */
import { Buffer } from 'node:buffer'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { decodeBase64, encodeBase64 } from './useBase64'

afterEach(() => vi.restoreAllMocks())

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

  it('大文本与标准编码字节一致，原生转换只接收小块', () => {
    const encode = vi.spyOn(globalThis, 'btoa')
    for (const tail of ['', 'a', 'ab', '😀']) {
      const text = '中文😀abc'.repeat(20_000) + tail
      expect(encodeBase64(text).output).toBe(Buffer.from(text, 'utf8').toString('base64'))
    }
    expect(encode.mock.calls.length).toBeGreaterThan(1)
    expect(encode.mock.calls.every(([chunk]) => chunk.length <= 0x7ffe)).toBe(true)
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

  it('跨分块的 UTF-8、BOM 与末尾残缺字符沿用严格解码语义', () => {
    const decode = vi.spyOn(globalThis, 'atob')
    const text = '\uFEFF' + 'a'.repeat(24_572) + '😀中文' + '\uFEFF' + 'b'.repeat(30_000)
    expect(decodeBase64(Buffer.from(text).toString('base64')).output).toBe(text.slice(1))
    expect(decode.mock.calls.every(([chunk]) => chunk.length <= 0x8000)).toBe(true)
    const incomplete = Buffer.concat([Buffer.from('a'.repeat(30_000)), Buffer.from([0xe4, 0xb8])])
    expect(decodeBase64(incomplete.toString('base64')).ok).toBe(false)
  })

  it('尾部 padding 不合法时不返回已完成分块的部分结果', () => {
    const prefix = 'YWFh'.repeat(10_000)
    for (const suffix of ['a', '===', 'YQ=a']) {
      expect(decodeBase64(prefix + suffix)).toMatchObject({ ok: false, output: '' })
    }
  })
})
