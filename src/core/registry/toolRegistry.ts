/**
 * 工具注册表 · 核心
 * 工具目录内自注册（tools/<id>/index.ts 调用 registerTool）；
 * 新增工具 = 建目录 + 注册一行，框架零改动（架构 §4.1）。
 *
 * 注册错误一律**立刻抛错**（T12-2）：重复 id、非法分类、重复设置键以前只是 warn 后覆盖，
 * 结果是「悄悄少了一个工具或一组设置」，问题要等用户发现才暴露。
 * 这里改成启动即失败，错误信息指明冲突双方，便于直接定位。
 *
 * 注册表可用 `createToolRegistry()` 实例化（测试与嵌入式场景互相隔离），
 * 默认导出实例供应用使用。
 */
import type { ToolManifest } from './types'

/** 合法分类（与 registry/types.ts 的 CategoryId 同步；此处做运行期校验） */
const VALID_CATEGORIES = ['dev', 'text', 'image', 'net', 'sys'] as const

/** 工具注册表（可实例化，避免全局单例在测试间互相污染） */
export interface ToolRegistry {
  /** 注册工具（重复 id / 非法分类 / 重复设置键都会抛错） */
  register(m: ToolManifest): void
  /** 全部工具（注册顺序） */
  all(): ToolManifest[]
  /** 按 id 查询 */
  get(id: string): ToolManifest | undefined
  /** 按分类聚合（侧栏分类计数由注册表统计，分类不是枚举） */
  byCategory(category: string): ToolManifest[]
  /** 各分类计数 */
  categoryCounts(): Record<string, number>
}

/** 校验清单：失败即抛错，信息包含冲突双方位置 */
function assertManifestValid(m: ToolManifest, existing: Map<string, ToolManifest>): void {
  if (!m.id.trim()) {
    throw new Error('[registry] 工具 id 不能为空')
  }
  const duplicate = existing.get(m.id)
  if (duplicate) {
    throw new Error(
      `[registry] 工具重复注册: ${m.id}（已注册：${duplicate.name}，重复项：${m.name}）`
    )
  }
  if (!(VALID_CATEGORIES as readonly string[]).includes(m.category)) {
    throw new Error(
      `[registry] 工具 ${m.id} 的分类非法: ${String(m.category)}（可用：${VALID_CATEGORIES.join(' / ')}）`
    )
  }
  if (!m.name.trim() || !m.icon.trim()) {
    throw new Error(`[registry] 工具 ${m.id} 缺少名称或图标`)
  }
  if (m.keywords.length === 0) {
    throw new Error(`[registry] 工具 ${m.id} 至少需要一个搜索关键词`)
  }
  const seen = new Set<string>()
  for (const field of m.settingsSchema ?? []) {
    if (seen.has(field.key)) {
      throw new Error(`[registry] 工具 ${m.id} 的设置键重复: ${field.key}`)
    }
    seen.add(field.key)
    if (field.type === 'select' && (!field.options || field.options.length === 0)) {
      throw new Error(`[registry] 工具 ${m.id} 的设置项 ${field.key} 是 select 但没有选项`)
    }
  }
}

/** 创建独立注册表实例 */
export function createToolRegistry(): ToolRegistry {
  const manifests = new Map<string, ToolManifest>()
  return {
    register(m: ToolManifest) {
      assertManifestValid(m, manifests)
      manifests.set(m.id, m)
    },
    all: () => [...manifests.values()],
    get: (id: string) => manifests.get(id),
    byCategory: (category: string) =>
      [...manifests.values()].filter((tool) => tool.category === category),
    categoryCounts() {
      const counts: Record<string, number> = {}
      for (const tool of manifests.values()) {
        counts[tool.category] = (counts[tool.category] ?? 0) + 1
      }
      return counts
    },
  }
}

/** 应用级默认注册表 */
export const toolRegistry = createToolRegistry()

/** 注册工具（应用启动时由各工具目录调用） */
export function registerTool(m: ToolManifest): void {
  toolRegistry.register(m)
}

export function getTools(): ToolManifest[] {
  return toolRegistry.all()
}

export function getTool(id: string): ToolManifest | undefined {
  return toolRegistry.get(id)
}

/** 按分类聚合（侧栏分类计数由注册表统计，分类不是枚举） */
export function getToolsByCategory(category: string): ToolManifest[] {
  return toolRegistry.byCategory(category)
}

export function getCategoryCounts(): Record<string, number> {
  return toolRegistry.categoryCounts()
}
