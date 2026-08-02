/**
 * 剪贴板历史 · 纯函数（搜索/排序/预览/相对时间）
 */
import type { ClipboardRecord } from "@/core/ipc/contracts";

/** 搜索 + 置顶优先排序 */
export function filterRecords(records: ClipboardRecord[], query: string): ClipboardRecord[] {
  const q = query.trim().toLowerCase();
  const filtered = q
    ? records.filter((r) => r.content.toLowerCase().includes(q))
    : [...records];
  // 置顶优先，其余按时间倒序（后端已按时间倒序，仅需稳定置顶）
  return filtered.sort((a, b) => Number(b.pinned) - Number(a.pinned));
}

/** 预览截断（多行取首行 + 长度限制） */
export function previewText(content: string, max = 120): string {
  const firstLine = content.split("\n")[0] ?? "";
  const trimmed = firstLine.length > max ? `${firstLine.slice(0, max)}…` : firstLine;
  return trimmed || "(空白内容)";
}

/** 相对时间：刚刚 / n 分钟前 / n 小时前 / n 天前 / 日期 */
export function formatTime(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 60_000) return "刚刚";
  const min = Math.floor(diff / 60_000);
  if (min < 60) return `${min} 分钟前`;
  const hour = Math.floor(min / 60);
  if (hour < 24) return `${hour} 小时前`;
  const day = Math.floor(hour / 24);
  if (day < 7) return `${day} 天前`;
  const d = new Date(ts);
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}
