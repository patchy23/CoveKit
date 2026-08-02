/**
 * HTTP/WS 调试 · 纯函数（请求构建/headers 解析/格式化）
 */
import type { HttpMethod } from "@/core/ipc/contracts";

export const METHODS: HttpMethod[] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

/** "Name: value" 多行文本 → kv 数组（忽略空行与无冒号行） */
export function parseHeaders(text: string): [string, string][] {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.includes(":"))
    .map((l) => {
      const i = l.indexOf(":");
      return [l.slice(0, i).trim(), l.slice(i + 1).trim()];
    });
}

/** kv 数组 → "Name: value" 多行文本 */
export function headersToText(headers: [string, string][]): string {
  return headers.map(([k, v]) => `${k}: ${v}`).join("\n");
}

/** 响应头 kv 数组 → 可读文本 */
export function formatHeaders(headers: [string, string][]): string {
  return headers.map(([k, v]) => `${k}: ${v}`).join("\n");
}

/** 字节数人类可读 */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

/** 响应体是否为 JSON（启发式） */
export function looksLikeJson(s: string): boolean {
  const t = s.trim();
  return t.startsWith("{") || t.startsWith("[") || t.startsWith('"');
}

/** URL 合法性（http/https/ws/wss 协议） */
export function isValidUrl(url: string): boolean {
  try {
    const u = new URL(url);
    return ["http:", "https:", "ws:", "wss:"].includes(u.protocol);
  } catch {
    return false;
  }
}
