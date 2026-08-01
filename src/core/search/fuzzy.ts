/**
 * 模糊搜索 · fuse.js 封装（名称 + 关键词 + 描述加权）
 * 索引由 tools store 在注册表聚合后初始化。
 */
import Fuse from "fuse.js";
import type { ToolManifest } from "@/core/registry/types";

let fuse: Fuse<ToolManifest> | null = null;

export function initSearch(tools: ToolManifest[]): void {
  fuse = new Fuse(tools, {
    keys: [
      { name: "name", weight: 0.5 },
      { name: "keywords", weight: 0.3 },
      { name: "description", weight: 0.2 },
    ],
    threshold: 0.38,
    ignoreLocation: true,
    includeMatches: true,
  });
}

/** 按查询词搜索工具；空查询返回空数组（上层自行决定展示全集） */
export function searchTools(query: string, limit = 50): ToolManifest[] {
  if (!fuse) return [];
  const q = query.trim();
  if (!q) return [];
  return fuse.search(q, { limit }).map((r) => r.item);
}

/** 带名称命中索引的搜索结果（用于卡片标题高亮） */
export interface ToolSearchHit {
  tool: ToolManifest;
  nameIndices: ReadonlyArray<readonly [number, number]>;
}

export function searchWithHits(query: string, limit = 50): ToolSearchHit[] {
  if (!fuse) return [];
  const q = query.trim();
  if (!q) return [];
  return fuse.search(q, { limit }).map((r) => ({
    tool: r.item,
    nameIndices: r.matches?.find((m) => m.key === "name")?.indices ?? [],
  }));
}

/** 高亮分片：命中片段用 <mark> 包裹（ToolCard 描述高亮） */
export interface HighlightChunk {
  text: string;
  hit: boolean;
}

export function highlightChunks(
  text: string,
  indices: ReadonlyArray<readonly [number, number]>
): HighlightChunk[] {
  if (!indices.length) return [{ text, hit: false }];
  const chunks: HighlightChunk[] = [];
  let cursor = 0;
  for (const [start, end] of indices) {
    if (start > cursor) chunks.push({ text: text.slice(cursor, start), hit: false });
    chunks.push({ text: text.slice(start, end + 1), hit: true });
    cursor = end + 1;
  }
  if (cursor < text.length) chunks.push({ text: text.slice(cursor), hit: false });
  return chunks;
}
