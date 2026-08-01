/**
 * UI 状态（Pinia）：当前分类 / 视图模式 / 搜索词 / 多页签工作区 / 设置弹窗 / 全局 toast
 */
import { defineStore } from "pinia";
import { ref } from "vue";

export const useUiStore = defineStore("ui", () => {
  /** 当前导航分类（"all" = 全部工具，"fav" = 我的收藏） */
  const activeCategory = ref<string>("all");
  /** 网格 / 列表视图 */
  const listView = ref(false);
  /** 顶栏搜索词 */
  const searchQuery = ref("");
  /** 设置弹窗 */
  const settingsVisible = ref(false);

  /* ── 多页签工作区（工具以子页面形式打开，可切换/关闭，状态保持）── */
  /** 已打开的工具 id（页签顺序） */
  const openTabs = ref<string[]>([]);
  /** 当前激活的工具 id（null = 工具库首页） */
  const activeTab = ref<string | null>(null);

  /** 打开工具页签（已打开则仅激活） */
  function openTool(id: string) {
    if (!openTabs.value.includes(id)) openTabs.value.push(id);
    activeTab.value = id;
  }

  /** 关闭页签；关闭激活页签时切到相邻页签或首页 */
  function closeTab(id: string) {
    const idx = openTabs.value.indexOf(id);
    if (idx < 0) return;
    openTabs.value = openTabs.value.filter((x) => x !== id);
    if (activeTab.value === id) {
      activeTab.value = openTabs.value[idx - 1] ?? openTabs.value[0] ?? null;
    }
  }

  /** 关闭全部工具页签，回到首页 */
  function closeAllTabs() {
    openTabs.value = [];
    activeTab.value = null;
  }

  /** 回到工具库首页 */
  function goHome() {
    activeTab.value = null;
  }

  /* ── 全局 toast（对齐原型 .toast，1600ms 自动消失）── */
  const toastMessage = ref("");
  const toastVisible = ref(false);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  function toast(msg: string) {
    toastMessage.value = msg;
    toastVisible.value = true;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastVisible.value = false;
    }, 1600);
  }

  return {
    activeCategory,
    listView,
    searchQuery,
    settingsVisible,
    openTabs,
    activeTab,
    openTool,
    closeTab,
    closeAllTabs,
    goHome,
    toastMessage,
    toastVisible,
    toast,
  };
});
