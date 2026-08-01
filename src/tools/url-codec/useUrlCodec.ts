/**
 * URL 编解码 · 纯函数（encodeURIComponent / decodeURIComponent）
 */
export interface CodecResult {
  ok: boolean;
  output: string;
  error?: string;
}

export function encodeUrl(text: string): CodecResult {
  return { ok: true, output: encodeURIComponent(text) };
}

export function decodeUrl(text: string): CodecResult {
  try {
    return { ok: true, output: decodeURIComponent(text.trim()) };
  } catch (err) {
    return {
      ok: false,
      output: "",
      error:
        err instanceof Error ? `解码失败：${err.message}` : "解码失败：输入不是合法的 URL 编码",
    };
  }
}

/** 判断文本是否包含疑似已编码片段（含 %xx） */
export function hasEncodedFragment(text: string): boolean {
  return /%(?:[0-9A-Fa-f]{2})/.test(text);
}
