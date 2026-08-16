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
    const bytes = new TextEncoder().encode(input)
    // 分块转二进制字符串，避免大文本 String.fromCharCode(...bytes) 爆栈
    let bin = ''
    const CHUNK = 0x8000
    for (let i = 0; i < bytes.length; i += CHUNK) {
      bin += String.fromCharCode(...bytes.subarray(i, i + CHUNK))
    }
    return { ok: true, output: btoa(bin) }
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
    const bin = atob(cleaned)
    const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0))
    // fatal: 非法 UTF-8 字节序列（多半是二进制文件内容）直接报错而不是乱码
    return { ok: true, output: new TextDecoder('utf-8', { fatal: true }).decode(bytes) }
  } catch (e) {
    return { ok: false, output: '', error: e instanceof Error ? e.message : String(e) }
  }
}
