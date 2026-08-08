/**
 * 应用设置（Pinia）：主题 / 语言 / 全局快捷键 / 自启 / 剪贴板策略 / 工具级设置
 * Tauri 环境经 IPC（settings_get/settings_set）读写 store 插件；非 Tauri 环境降级 storage 适配层。
 */
import { defineStore } from "pinia";
import { ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ipc } from "@/core/ipc/ipc";
import type { AppSettings } from "@/core/ipc/contracts";
import { storage } from "@/core/storage";

const STORE_FILE = "patchybox.json";
const SETTINGS_KEY = "settings";

const DEFAULTS: AppSettings = {
  theme: "system",
  language: "zh-CN",
  globalHotkey: "Ctrl+Shift+Space",
  launchAtStartup: false,
  recentTools: [],
  tools: {},
};

export const useSettingsStore = defineStore("settings", () => {
  const settings = ref<AppSettings>({ ...DEFAULTS });
  const loaded = ref(false);

  /** 应用主题到 <html data-theme>（@custom-variant dark 绑定）+ Windows 标题栏同步 */
  function applyTheme() {
    const { theme } = settings.value;
    const dark =
      theme === "dark" ||
      (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    document.documentElement.dataset.theme = dark ? "dark" : "";
    // 原生标题栏跟随主题（WebView 环境才可用；浏览器预览忽略）
    if ("__TAURI_INTERNALS__" in window) {
      getCurrentWindow()
        .setTheme(dark ? "dark" : "light")
        .catch(() => {});
    }
  }

  async function init() {
    try {
      const remote = await ipc.settingsGet();
      settings.value = { ...DEFAULTS, ...remote };
    } catch {
      // 非 Tauri 环境：降级 storage 适配层
      const local = await storage.get<Partial<AppSettings>>(STORE_FILE, SETTINGS_KEY);
      settings.value = { ...DEFAULTS, ...(local ?? {}) };
    }
    applyTheme();
    loaded.value = true;
  }

  async function set<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    settings.value[key] = value;
    if (key === "theme") applyTheme();
    if (key === "language") document.documentElement.lang = String(value);
    try {
      await ipc.settingsSet(key as string, value);
    } catch {
      await storage.set(STORE_FILE, SETTINGS_KEY, settings.value);
    }
  }

  /* ── 工具级设置（settingsSchema 渲染 + 按 id 分区存储）── */

  function getToolSetting<T>(toolId: string, key: string, fallback: T): T {
    return (settings.value.tools[toolId]?.[key] as T | undefined) ?? fallback;
  }

  async function setToolSetting(toolId: string, key: string, value: unknown) {
    settings.value.tools = {
      ...settings.value.tools,
      [toolId]: { ...(settings.value.tools[toolId] ?? {}), [key]: value },
    };
    await set("tools", settings.value.tools);
  }

  return {
    settings,
    loaded,
    init,
    set,
    applyTheme,
    getToolSetting,
    setToolSetting,
  };
});
