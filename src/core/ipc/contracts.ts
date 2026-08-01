/**
 * IPC 契约 · 唯一事实源
 * 与 src-tauri/modules 的 serde 结构同步（架构 §5）。
 * 铁律：IPC 出入参只在本文档出现一次；
 *       新增 Rust 命令 = 在此声明 payload/result + Rust 模块实现 + lib.rs 装配一行。
 */

// ── 出参结构 ──

/** 剪贴板历史记录 */
export interface ClipboardRecord {
  id: string;
  kind: "text" | "image" | "file";
  content: string;
  preview: string;
  pinned: boolean;
  createdAt: number;
}

/** 应用设置（Rust modules/settings.rs 全量读写） */
export interface AppSettings {
  theme: "light" | "dark" | "system";
  language: "zh-CN" | "en-US";
  globalHotkey: string;
  launchAtStartup: boolean;
  clipboard: { enabled: boolean; historyLimit: number; ignore: string[] };
  recentTools: string[];
  tools: Record<string, Record<string, unknown>>;
}

/** 屏幕取色结果 */
export interface PickColorResult {
  hex: string;
  rgb: [number, number, number];
}

/** 窗口状态 */
export interface WindowState {
  visible: boolean;
}

// ── 命令清单（IPC 出入参的唯一出处）──

export const commandNames = {
  settingsGet: "settings_get",
  settingsSet: "settings_set",
  clipboardList: "clipboard_list",
  clipboardDelete: "clipboard_delete",
  clipboardClear: "clipboard_clear",
  clipboardTogglePin: "clipboard_toggle_pin",
  colorPickScreen: "color_pick_screen",
  windowToggle: "window_toggle",
  windowHide: "window_hide",
  openExternal: "open_external",
} as const;

/** 各命令入参（Record<string, never> = 无参命令） */
export type IpcPayloads = {
  settings_get: { key?: string };
  settings_set: { key: string; value: unknown };
  clipboard_list: { limit?: number; pinnedOnly?: boolean };
  clipboard_delete: { id: string };
  clipboard_clear: Record<string, never>;
  clipboard_toggle_pin: { id: string };
  color_pick_screen: Record<string, never>;
  window_toggle: Record<string, never>;
  window_hide: Record<string, never>;
  open_external: { url: string };
};

/** 各命令返回 */
export type IpcResults = {
  settings_get: AppSettings;
  settings_set: void;
  clipboard_list: ClipboardRecord[];
  clipboard_delete: void;
  clipboard_clear: void;
  clipboard_toggle_pin: void;
  color_pick_screen: PickColorResult;
  window_toggle: WindowState;
  window_hide: void;
  open_external: void;
};
