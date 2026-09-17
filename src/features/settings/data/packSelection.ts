/**
 * 导出选择与依赖闭包预览（sync L2 · 纯函数）
 *
 * 为什么单独一层：向导的「下一步」是否可用、以及第 2 步要展示的「连带带出什么」，
 * 都必须能从目录数据直接算出来，而不是等后端开跑才知道。算错了用户看到的是空承诺。
 *
 * 口径（与 Rust 侧 `catalog::resolve_selection` 对齐，本文件只做**预览**，不做授权）：
 * - 只有 `selectable` 的数据集能被勾选；
 * - 凭证/分组/隧道/书签按**所选档案的引用**带出，不单独勾选；
 * - 预览只统计条数，最终以 Rust 返回的报告为准（前端不与后端争真相）。
 */
import type { ExportCatalog, ExportCatalogEntry, ExportSelection } from '@/core/ipc/contracts'

/** 依赖边类型（与 Rust 侧 `DependencyEdge.kind` 逐字对应） */
export type DependencyKind = 'credential' | 'group' | 'tunnel' | 'bookmark' | 'profile' | string

/** 依赖类型 → i18n 键后缀（展示名不在纯函数里拼，界面负责翻译） */
export const DEPENDENCY_LABEL_KEYS: Record<string, string> = {
  credential: 'credential',
  group: 'group',
  tunnel: 'tunnel',
  bookmark: 'bookmark',
  profile: 'profile',
}

/** 勾选态（向导第 1 步的界面状态） */
export interface ExportChoice {
  /** 勾选的服务器档案 id */
  profileIds: string[]
  /** 是否带上收藏 */
  includeFavorites: boolean
  /** 是否带上最近使用 */
  includeRecentTools: boolean
}

/** 初始勾选态：收藏默认带上、最近使用默认不带（方案 §13.1） */
export function initialChoice(catalog: ExportCatalog | null): ExportChoice {
  const defaults = catalog?.defaults
  return {
    profileIds: [],
    includeFavorites: defaults ? defaults.datasets.includes('core.favorites') : true,
    includeRecentTools: defaults ? defaults.datasets.includes('core.recent_tools') : false,
  }
}

/** 可勾选的档案条目（按目录声明：只有 selectable 的数据集能被勾） */
export function profileEntries(catalog: ExportCatalog | null): ExportCatalogEntry[] {
  if (!catalog) return []
  const selectable = new Set(
    catalog.datasets.filter((item) => item.selectable).map((item) => item.name)
  )
  return catalog.entries.filter((entry) => selectable.has(entry.dataset))
}

/** 组装成 IPC 入参（Rust 侧按数据集名分组，未知数据集/未知 id 会被拒绝） */
export function buildSelection(
  catalog: ExportCatalog | null,
  choice: ExportChoice
): ExportSelection {
  const datasets: string[] = []
  if (choice.includeFavorites) datasets.push('core.favorites')
  if (choice.includeRecentTools) datasets.push('core.recent_tools')

  const selectable = new Set(
    (catalog?.datasets ?? []).filter((item) => item.selectable).map((item) => item.name)
  )
  const byDataset = new Map<string, string[]>()
  for (const id of choice.profileIds) {
    const entry = (catalog?.entries ?? []).find(
      (item) => item.id === id && selectable.has(item.dataset)
    )
    if (!entry) continue
    const ids = byDataset.get(entry.dataset) ?? []
    ids.push(id)
    byDataset.set(entry.dataset, ids)
  }

  return {
    entries: [...byDataset.entries()].map(([dataset, ids]) => ({ dataset, ids })),
    datasets,
  }
}

/** 闭包预览：按依赖类型统计将被连带带出的条数（去重） */
export function closurePreview(
  catalog: ExportCatalog | null,
  choice: ExportChoice
): { kind: DependencyKind; count: number }[] {
  const chosen = new Set(choice.profileIds)
  const perKind = new Map<string, Set<string>>()

  for (const entry of profileEntries(catalog)) {
    if (!chosen.has(entry.id)) continue
    for (const edge of entry.dependencies) {
      // 分组/隧道/书签随档案带出；凭证永不进包（与空间绑定），但引用关系仍展示
      const set = perKind.get(edge.kind) ?? new Set<string>()
      set.add(edge.toId)
      perKind.set(edge.kind, set)
    }
  }

  return [...perKind.entries()]
    .map(([kind, ids]) => ({ kind, count: ids.size }))
    .sort((a, b) => a.kind.localeCompare(b.kind))
}

/** 已选条目的提示（例如「未绑定凭证，导入后需补全」） */
export function chosenNotes(
  catalog: ExportCatalog | null,
  choice: ExportChoice
): ExportCatalogEntry[] {
  const chosen = new Set(choice.profileIds)
  return profileEntries(catalog).filter((entry) => chosen.has(entry.id) && entry.note)
}

/** 依赖类别 → i18n 键（界面直接用；不要在各处再拼一次字符串） */
export function dependencyLabelKey(kind: DependencyKind): string {
  const suffix = DEPENDENCY_LABEL_KEYS[kind] ?? 'profile'
  return `settings.dataManagement.dep${suffix.charAt(0).toUpperCase()}${suffix.slice(1)}`
}
