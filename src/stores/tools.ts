/**
 * 工具状态（Pinia）：注册表聚合 / 分类过滤 / 模糊搜索 / 最近使用
 * 工具本体只负责注册；展示、过滤、排序全部由本 store 承担。
 */
import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";
import { getCategoryCounts, getTools } from "@/core/registry/toolRegistry";
import type { ToolManifest } from "@/core/registry/types";
import { highlightChunks, initSearch, searchWithHits } from "@/core/search/fuzzy";
import { storage } from "@/core/storage";
import { useUiStore } from "./ui";
import { useFavoritesStore } from "./favorites";

const STORE_FILE = "patchybox.json";
const RECENT_KEY = "recent";
const RECENT_LIMIT = 6;

export const useToolsStore = defineStore("tools", () => {
  const tools = ref<ToolManifest[]>(getTools());
  const recent = ref<string[]>([]);
  /** 搜索结果名称高亮索引：toolId → name 命中区间 */
  const nameHits = ref<Record<string, ReadonlyArray<readonly [number, number]>>>({});

  const ui = useUiStore();
  const favorites = useFavoritesStore();

  initSearch(tools.value);

  /** 分类计数（侧栏徽标） */
  const categoryCounts = computed(() => {
    const counts = getCategoryCounts();
    counts.all = tools.value.length;
    counts.fav = favorites.ids.length;
    return counts;
  });

  /** 当前视图工具列表（分类 + 收藏 + 搜索 联合过滤） */
  const filtered = computed<ToolManifest[]>(() => {
    let list = tools.value;
    if (ui.activeCategory === "fav") {
      list = list.filter((t) => favorites.has(t.id));
    } else if (ui.activeCategory !== "all") {
      list = list.filter((t) => t.category === ui.activeCategory);
    }
    const q = ui.searchQuery.trim();
    if (q) {
      const hitIds = new Set(searchWithHits(q).map((h) => h.tool.id));
      list = list.filter((t) => hitIds.has(t.id));
    }
    return list;
  });

  /** 搜索词变化时维护名称高亮索引 */
  watch(
    () => ui.searchQuery,
    (q) => {
      nameHits.value = {};
      const trimmed = q.trim();
      if (trimmed) {
        for (const hit of searchWithHits(trimmed)) {
          if (hit.nameIndices.length) nameHits.value[hit.tool.id] = hit.nameIndices;
        }
      }
    }
  );

  /** 卡片名称高亮分片 */
  function nameChunks(tool: ToolManifest) {
    const indices = nameHits.value[tool.id];
    return indices?.length ? highlightChunks(tool.name, indices) : null;
  }

  /* ── 最近使用 ── */

  async function initRecent() {
    recent.value = (await storage.get<string[]>(STORE_FILE, RECENT_KEY)) ?? [];
  }

  async function pushRecent(id: string) {
    recent.value = [id, ...recent.value.filter((x) => x !== id)].slice(0, RECENT_LIMIT);
    await storage.set(STORE_FILE, RECENT_KEY, recent.value);
  }

  /** 打开工具：记录最近使用 + 打开弹窗 */
  async function openTool(id: string) {
    ui.openToolId = id;
    await pushRecent(id);
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
  };
});
