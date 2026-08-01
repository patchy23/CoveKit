/**
 * Markdown 预览 · 纯函数（marked GFM 渲染 + 基础 HTML 净化）
 * M2 引入 DOMPurify 做完整 sanitize；当前输入为本地文本，剥离高风险标签即可。
 */
import { marked } from "marked";

marked.setOptions({
  gfm: true,
  breaks: true,
});

/** 剥离脚本/iframe/事件属性等高风险内容 */
export function sanitizeHtml(html: string): string {
  return html
    .replace(/<script[\s\S]*?<\/script>/gi, "")
    .replace(/<iframe[\s\S]*?<\/iframe>/gi, "")
    .replace(/<style[\s\S]*?<\/style>/gi, "")
    .replace(/\son\w+\s*=\s*"[^"]*"/gi, "")
    .replace(/javascript:/gi, "");
}

export function renderMarkdown(text: string): string {
  const raw = marked.parse(text, { async: false }) as string;
  return sanitizeHtml(raw);
}
