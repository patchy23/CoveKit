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

/* ── Postman 式键值行（Params / Headers 表格） ── */

export interface KvRow {
  id: string;
  key: string;
  value: string;
}

let kvSeq = 0;
/** 生成行 id */
export function newKvId(): string {
  kvSeq += 1;
  return `kv${Date.now().toString(36)}${kvSeq}`;
}

/** kv 行 → 请求头对象（空 key 忽略） */
export function kvToHeaders(rows: KvRow[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const r of rows) {
    const k = r.key.trim();
    if (k) out[k] = r.value;
  }
  return out;
}

/** kv 行 → query 字符串（"a=1&b=2"，值 encode） */
export function kvToQuery(rows: KvRow[]): string {
  return rows
    .filter((r) => r.key.trim())
    .map((r) => `${encodeURIComponent(r.key.trim())}=${encodeURIComponent(r.value)}`)
    .join("&");
}

/** 合并 query 到 URL（已有 query 追加 &） */
export function mergeQuery(url: string, query: string): string {
  if (!query) return url;
  const sep = url.includes("?") ? "&" : "?";
  return `${url}${sep}${query}`;
}

/** 相对时间（历史列表用） */
export function formatRelativeTime(iso: string): string {
  const t = new Date(iso.replace(" ", "T")).getTime();
  if (Number.isNaN(t)) return iso.slice(5, 19).replace("T", " ");
  const diff = Date.now() - t;
  const min = Math.floor(diff / 60000);
  if (min < 1) return "刚刚";
  if (min < 60) return `${min} 分钟前`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h} 小时前`;
  return `${Math.floor(h / 24)} 天前`;
}

/** 接口草稿（面板与接口列表之间的统一数据契约） */
export interface ApiDraft {
  type: "http" | "ws";
  method: string;
  url: string;
  params: KvRow[];
  headers: KvRow[];
  bodyMode: "none" | "json" | "text";
  body: string;
}

/** KvRow ↔ "Name: Value" 文本 */
export function kvToText(rows: KvRow[]): string {
  return rows
    .filter((r) => r.key.trim())
    .map((r) => `${r.key.trim()}: ${r.value}`)
    .join("\n");
}

export function textToKv(text: string): KvRow[] {
  return parseHeaders(text).map(([k, v]) => ({ id: newKvId(), key: k, value: v }));
}

/** 方法徽标配色（Apifox 惯例：GET 绿 / POST 橙 / PUT 蓝 / PATCH 紫 / DELETE 红 / HEAD 灰 / OPTIONS 紫 / WS 青） */
export function methodBadgeClass(method: string, type?: string): string {
  if (type === "ws")
    return "bg-cyan-soft text-cyan-strong dark:bg-cyan-soft-dark dark:text-cyan-dark";
  switch (method) {
    case "GET":
      return "bg-success-soft text-success-strong dark:bg-success-soft-dark dark:text-success-dark";
    case "POST":
      return "bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark";
    case "PUT":
      return "bg-info-soft text-info-strong dark:bg-info-soft-dark dark:text-info-dark";
    case "PATCH":
      return "bg-purple-soft text-purple-strong dark:bg-purple-soft-dark dark:text-purple-dark";
    case "DELETE":
      return "bg-danger-soft text-danger-strong dark:bg-danger-soft-dark dark:text-danger-dark";
    case "OPTIONS":
      return "bg-purple-soft text-purple-strong dark:bg-purple-soft-dark dark:text-purple-dark";
    default:
      return "bg-neutral text-secondary dark:bg-neutral-dark dark:text-secondary-dark";
  }
}

/** 方法纯文字色（select 值区用：仅文字变色，保持默认背景） */
export function methodTextClass(method: string, type?: string): string {
  if (type === "ws") return "text-cyan-strong dark:text-cyan-dark";
  switch (method) {
    case "GET":
      return "text-success-strong dark:text-success-dark";
    case "POST":
      return "text-tertiary-strong dark:text-tertiary-dark";
    case "PUT":
      return "text-info-strong dark:text-info-dark";
    case "PATCH":
      return "text-purple-strong dark:text-purple-dark";
    case "DELETE":
      return "text-danger-strong dark:text-danger-dark";
    case "OPTIONS":
      return "text-purple-strong dark:text-purple-dark";
    default:
      return "text-secondary dark:text-secondary-dark";
  }
}
