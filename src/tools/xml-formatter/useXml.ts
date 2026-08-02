/**
 * XML 格式化 · 纯函数（DOMParser 校验 + 缩进重排 + 压缩）
 */

export interface XmlResult {
  ok: boolean;
  output?: string;
  error?: string;
}

/** 格式化：解析校验 → 序列化 → 按标签缩进重排 */
export function formatXml(xml: string, indent = 2): XmlResult {
  const trimmed = xml.trim();
  if (!trimmed) return { ok: false, error: "输入为空" };
  const doc = new DOMParser().parseFromString(trimmed, "text/xml");
  const err = doc.querySelector("parsererror");
  if (err) {
    return { ok: false, error: (err.textContent ?? "XML 解析失败").trim().slice(0, 200) };
  }
  const text = new XMLSerializer().serializeToString(doc);
  return { ok: true, output: prettyPrint(text, indent) };
}

/** 压缩：去掉标签间空白与首尾空白 */
export function minifyXml(xml: string): string {
  return xml.replace(/>\s+</g, "><").trim();
}

/** 校验是否为合法 XML */
export function isValidXml(xml: string): boolean {
  if (!xml.trim()) return false;
  const doc = new DOMParser().parseFromString(xml, "text/xml");
  return !doc.querySelector("parsererror");
}

/** 按标签深度缩进重排（保留声明/注释；文本节点内联到标签行） */
function prettyPrint(xml: string, indent: number): string {
  const tokens = xml.match(/<\?[^>]*\?>|<!--[\s\S]*?-->|<\/?[^>]+>|[^<]+/g) ?? [];
  const pad = " ".repeat(indent);
  let depth = 0;
  let pendingText = false;
  const lines: string[] = [];
  for (const tok of tokens) {
    if (/^\s*$/.test(tok)) continue;
    if (tok.startsWith("<?") || tok.startsWith("<!--")) {
      lines.push(pad.repeat(depth) + tok);
    } else if (tok.startsWith("</")) {
      depth = Math.max(0, depth - 1);
      if (pendingText) {
        lines[lines.length - 1] += tok;
        pendingText = false;
      } else {
        lines.push(pad.repeat(depth) + tok);
      }
    } else if (tok.startsWith("<")) {
      lines.push(pad.repeat(depth) + tok);
      if (!tok.endsWith("/>")) depth += 1;
    } else {
      const t = tok.trim();
      if (t) {
        // 文本节点：内联到上一个标签行（<c>text</c> 同行）
        lines[lines.length - 1] += t;
        pendingText = true;
      }
    }
  }
  return lines.join("\n");
}
