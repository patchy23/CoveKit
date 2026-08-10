/**
 * XML 格式化 · 纯函数（DOMParser 校验 + 缩进重排 + 压缩）
 */

export interface XmlResult {
  ok: boolean
  output?: string
  error?: string
  /** 宽松模式：严格解析失败（如 @url: 伪命名空间）时按标签缩进重排，未做结构校验 */
  loose?: boolean
}

/** 格式化：解析校验 → 序列化 → 按标签缩进重排；严格解析失败时回退宽松格式化 */
export function formatXml(xml: string, indent = 2): XmlResult {
  const trimmed = xml.trim()
  if (!trimmed) return { ok: false, error: '输入为空' }
  // CDATA 占位：先替换为注释再解析（兼容解析器差异，序列化后还原）
  const cdata: string[] = []
  const sanitized = trimmed.replace(/<!\[CDATA\[[\s\S]*?\]\]>/g, (m) => {
    cdata.push(m)
    return `<!--__CDATA_${cdata.length - 1}__-->`
  })
  const doc = new DOMParser().parseFromString(sanitized, 'text/xml')
  const err = doc.querySelector('parsererror')
  if (err) {
    const msg = (err.textContent ?? '').toLowerCase()
    // 仅命名空间/URI 类错误（如 xmlns="@url:..." 伪 URI）回退宽松格式化；
    // 结构错误（标签不闭合等）仍报错提示
    if (msg.includes('uri') || msg.includes('namespace')) {
      return { ok: true, output: prettyPrint(trimmed, indent), loose: true }
    }
    return { ok: false, error: (err.textContent ?? 'XML 解析失败').trim().slice(0, 200) }
  }
  let text = new XMLSerializer().serializeToString(doc)
  text = text.replace(/<!--__CDATA_(\d+)__-->/g, (_, i) => cdata[Number(i)] ?? '')
  return { ok: true, output: prettyPrint(text, indent) }
}

/** 压缩：去掉标签间空白与首尾空白 */
export function minifyXml(xml: string): string {
  return xml.replace(/>\s+</g, '><').trim()
}

/** 校验是否为合法 XML */
export function isValidXml(xml: string): boolean {
  if (!xml.trim()) return false
  const sanitized = xml.replace(/<!\[CDATA\[[\s\S]*?\]\]>/g, 'x')
  const doc = new DOMParser().parseFromString(sanitized, 'text/xml')
  return !doc.querySelector('parsererror')
}

/** 按标签深度缩进重排（保留声明/注释；CDATA 作为整体内联；文本节点内联到标签行） */
function prettyPrint(xml: string, indent: number): string {
  // CDATA 优先整体匹配（内部含 < > 不能被当标签切碎），其次声明/注释/标签/文本
  const tokens =
    xml.match(/<!\[CDATA\[[\s\S]*?\]\]>|<\?[^>]*\?>|<!--[\s\S]*?-->|<\/?[^>]+>|[^<]+/g) ?? []
  const pad = ' '.repeat(indent)
  let depth = 0
  let pendingText = false
  const lines: string[] = []
  for (const tok of tokens) {
    if (/^\s*$/.test(tok)) continue
    if (tok.startsWith('<?') || tok.startsWith('<!--')) {
      lines.push(pad.repeat(depth) + tok)
    } else if (tok.startsWith('<![CDATA[')) {
      // CDATA：内容节点，不改变深度，内联到上一个标签行
      lines[lines.length - 1] += tok
      pendingText = true
    } else if (tok.startsWith('</')) {
      depth = Math.max(0, depth - 1)
      if (pendingText) {
        lines[lines.length - 1] += tok
        pendingText = false
      } else {
        lines.push(pad.repeat(depth) + tok)
      }
    } else if (tok.startsWith('<')) {
      lines.push(pad.repeat(depth) + tok)
      if (!tok.endsWith('/>')) depth += 1
    } else {
      const t = tok.trim()
      if (t) {
        // 文本节点：内联到上一个标签行（<c>text</c> 同行）
        lines[lines.length - 1] += t
        pendingText = true
      }
    }
  }
  return lines.join('\n')
}
