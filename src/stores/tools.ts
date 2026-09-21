/**
 * 工具状态（Pinia）：注册表聚合 / 分类过滤 / 模糊搜索 / 最近使用
 * 工具本体只负责注册；展示、过滤、排序全部由本 store 承担。
 */
import { isTauri } from '@tauri-apps/api/core'
import { warn as logWarn } from '@tauri-apps/plugin-log'
import { defineStore } from 'pinia'
import { useDataRefresh } from '@/core/dataTransfer/useDataRefresh'
import { computed, ref, watch } from 'vue'
import { getCategoryCounts, getTools } from '@/core/registry/toolRegistry'
import type { ToolManifest } from '@/core/registry/types'
import { highlightChunks, initSearch, searchWithHits } from '@/core/search/fuzzy'
import { isStringArray, spaceData } from '@/core/spaceData'
import { useUiStore } from './ui'
import { useFavoritesStore } from './favorites'

/** 最近使用键（与 Rust 白名单一致；不再复用前端历史键名 `recent`） */
const RECENT_KEY = 'recentTools'
const RECENT_LIMIT = 6

export const useToolsStore = defineStore('tools', () => {
  const tools = ref<ToolManifest[]>(getTools())
  const recent = ref<string[]>([])
  /** 搜索结果名称高亮索引：toolId → name 命中区间 */
  const nameHits = ref<Record<string, ReadonlyArray<readonly [number, number]>>>({})

  const ui = useUiStore()
  useDataRefresh('core.recent_tools', initRecent)
  const favorites = useFavoritesStore()

  initSearch(tools.value)

  /** 分类计数（侧栏徽标） */
  const categoryCounts = computed(() => {
    const counts = getCategoryCounts()
    counts.all = tools.value.length
    counts.fav = favorites.ids.length
    return counts
  })

  /** 当前视图工具列表（分类 + 收藏 + 搜索 联合过滤） */
  const filtered = computed<ToolManifest[]>(() => {
    let list = tools.value
    if (ui.activeCategory === 'fav') {
      list = list.filter((t) => favorites.has(t.id))
    } else if (ui.activeCategory !== 'all') {
      list = list.filter((t) => t.category === ui.activeCategory)
    }
    const q = ui.searchQuery.trim()
    if (q) {
      const hitIds = new Set(searchWithHits(q).map((h) => h.tool.id))
      list = list.filter((t) => hitIds.has(t.id))
    }
    return list
  })

  /** 搜索词变化时维护名称高亮索引 */
  watch(
    () => ui.searchQuery,
    (q) => {
      nameHits.value = {}
      const trimmed = q.trim()
      if (trimmed) {
        for (const hit of searchWithHits(trimmed)) {
          if (hit.nameIndices.length) nameHits.value[hit.tool.id] = hit.nameIndices
        }
      }
    }
  )

  /** 卡片名称高亮分片 */
  function nameChunks(tool: ToolManifest) {
    const indices = nameHits.value[tool.id]
    return indices?.length ? highlightChunks(tool.name, indices) : null
  }

  /* ── 最近使用 ── */

  async function initRecent() {
    recent.value = (await spaceData.get<string[]>(RECENT_KEY, isStringArray)) ?? []
  }

  /**
   * 记录最近使用
   *
   * 写入失败只记控制台并保留内存值：最近使用是打开工具时的附带记账，
   * 失败不该升级成「工具打不开」；收藏是用户主动操作，走 favorites store 的回滚 + 报错路径。
   */
  async function pushRecent(id: string) {
    recent.value = [id, ...recent.value.filter((x) => x !== id)].slice(0, RECENT_LIMIT)
    try {
      await spaceData.set(RECENT_KEY, recent.value)
    } catch (error) {
      if (isTauri()) {
        void logWarn('工具使用记录写入失败 source=tools').catch(() => {
          console.warn('[diagnostics] 日志发送失败')
        })
      }
      console.error('[tools] 最近使用写入失败（工具已打开，仅记账未落盘）', error)
    }
  }

  /** 打开工具：打开页签 + 记录最近使用 */
  async function openTool(id: string) {
    ui.openTool(id)
    await pushRecent(id)
  }

  return {
    tools,
    recent,
    nameHits,
    categoryCounts,
    filtered,
    nameChunks,
    initRecent,
    pushRecent,
    openTool,
  }
})
