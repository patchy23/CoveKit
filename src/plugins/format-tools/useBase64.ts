/**
 * Base64 编解码 · 纯函数（UTF-8 安全：btoa/atob 只支持 Latin-1，中文走 TextEncoder/TextDecoder）
 */

export interface Base64Result {
  ok: boolean
  output: string
  error?: string
}

/** 文本 → Base64（UTF-8 字节编码） */
export function encodeBase64(input: string): Base64Result {
  if (!input) return { ok: false, output: '', error: '请输入要编码的文本' }
  try {
    const encoder = new TextEncoder()
    const CHARS = 0x2000
    // 小文本保持直接路径；大文本同时限制 UTF-8 临时缓冲和 btoa 的参数块。
    if (input.length <= CHARS)
      return { ok: true, output: btoa(String.fromCharCode(...encoder.encode(input))) }
    // 每个 UTF-16 码元至多编码为 3 字节，再预留 2 字节 Base64 尾部，encodeInto 总能读完整块。
    const bytes = new Uint8Array(CHARS * 3 + 2)
    let position = 0,
      carry = 0
    let output = ''
    while (position < input.length) {
      let end = Math.min(position + CHARS, input.length)
      // 不拆 UTF-16 代理对；独立的非法代理项仍交给 TextEncoder 按原语义替换。
      const last = input.charCodeAt(end - 1),
        next = input.charCodeAt(end)
      if (
        end < input.length &&
        last >= 0xd800 &&
        last <= 0xdbff &&
        next >= 0xdc00 &&
        next <= 0xdfff
      )
        end--
      const chunk = input.slice(position, end)
      let written: number
      if (encoder.encodeInto) written = encoder.encodeInto(chunk, bytes.subarray(carry)).written
      else {
        const encoded = encoder.encode(chunk)
        bytes.set(encoded, carry)
        written = encoded.length
      }
      const length = carry + written
      const complete = length - (length % 3)
      output += btoa(String.fromCharCode(...bytes.subarray(0, complete)))
      carry = length - complete
      bytes.copyWithin(0, complete, length)
      position = end
    }
    if (carry) output += btoa(String.fromCharCode(...bytes.subarray(0, carry)))
    return { ok: true, output }
  } catch (e) {
    return { ok: false, output: '', error: e instanceof Error ? e.message : String(e) }
  }
}

/** Base64 → 文本（容忍空白/换行；非法字符或 UTF-8 解码失败时明确报错） */
export function decodeBase64(input: string): Base64Result {
  const cleaned = input.replace(/\s+/g, '')
  if (!cleaned) return { ok: false, output: '', error: '请输入要解码的 Base64' }
  if (!/^[A-Za-z0-9+/]*={0,2}$/.test(cleaned)) {
    return { ok: false, output: '', error: '包含非法字符，不是有效的 Base64' }
  }
  try {
    // fatal: 非法 UTF-8 字节序列（多半是二进制文件内容）直接报错而不是乱码
    const decoder = new TextDecoder('utf-8', { fatal: true })
    let output = ''
    // 4 字符对齐保持 Base64 字节边界；UTF-8 跨块字符由同一 decoder 保留尾部。
    const CHUNK = 0x8000
    for (let i = 0; i < cleaned.length; i += CHUNK) {
      const bin = atob(cleaned.slice(i, i + CHUNK))
      const bytes = new Uint8Array(bin.length)
      for (let j = 0; j < bin.length; j++) bytes[j] = bin.charCodeAt(j)
      output += decoder.decode(bytes, { stream: true })
    }
    output += decoder.decode()
    return { ok: true, output }
  } catch (e) {
    return { ok: false, output: '', error: e instanceof Error ? e.message : String(e) }
  }
}
