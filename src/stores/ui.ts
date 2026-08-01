/**
 * UI 状态（Pinia）：当前分类 / 视图模式 / 搜索词 / 弹窗开关 / 全局 toast
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
  /** 当前打开的弹窗工具 id（null = 无弹窗） */
  const openToolId = ref<string | null>(null);
  /** 设置弹窗 */
  const settingsVisible = ref(false);

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
    openToolId,
    settingsVisible,
    toastMessage,
    toastVisible,
    toast,
  };
});
