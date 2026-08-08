/**
 * IPC 契约 · 框架级（窗口 / 设置 / 外链）
 * 业务插件的 IPC 契约定义在各插件目录 src/plugins/<id>/contracts.ts
 * （插件隔离：新增/修改插件契约不动本文件与其它插件）。
 */

// ── 出参结构 ──

/** 应用设置（Rust framework/settings.rs 全量读写） */
export interface AppSettings {
  theme: "light" | "dark" | "system";
  language: "zh-CN" | "en-US";
  globalHotkey: string;
  launchAtStartup: boolean;
  recentTools: string[];
  tools: Record<string, Record<string, unknown>>;
}

/** 窗口状态 */
export interface WindowState {
  visible: boolean;
}

// ── 框架命令清单 ──

export const frameworkCommands = {
  settingsGet: "settings_get",
  settingsSet: "settings_set",
  windowToggle: "window_toggle",
  windowHide: "window_hide",
  openExternal: "open_external",
  frameworkCommandsList: "framework_commands",
} as const;

/** 框架命令入参（Record<string, never> = 无参命令） */
export type FrameworkPayloads = {
  settings_get: { key?: string };
  settings_set: { key: string; value: unknown };
  window_toggle: Record<string, never>;
  window_hide: Record<string, never>;
  open_external: { url: string };
  framework_commands: Record<string, never>;
};

/** 框架命令返回 */
export type FrameworkResults = {
  settings_get: AppSettings;
  settings_set: void;
  window_toggle: WindowState;
  window_hide: void;
  open_external: void;
  framework_commands: { name: string; doc: string }[];
};
