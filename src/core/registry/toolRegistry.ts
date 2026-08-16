/**
 * 工具注册表 · 核心
 * 工具目录内自注册（tools/<id>/index.ts 调用 registerTool）；
 * 新增工具 = 建目录 + 注册一行，框架零改动（架构 §4.1）。
 */
import type { ToolManifest } from './types'

const manifests = new Map<string, ToolManifest>()

export function registerTool(m: ToolManifest): void {
  if (manifests.has(m.id)) {
    console.warn(`[registry] 工具重复注册: ${m.id}`)
  }
  manifests.set(m.id, m)
}

export function getTools(): ToolManifest[] {
  return [...manifests.values()]
}

export function getTool(id: string): ToolManifest | undefined {
  return manifests.get(id)
}

/** 按分类聚合（侧栏分类计数由注册表统计，分类不是枚举） */
export function getToolsByCategory(category: string): ToolManifest[] {
  return getTools().filter((t) => t.category === category)
}

export function getCategoryCounts(): Record<string, number> {
  const counts: Record<string, number> = {}
  for (const t of getTools()) {
    counts[t.category] = (counts[t.category] ?? 0) + 1
  }
  return counts
}
