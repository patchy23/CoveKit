/**
 * IPC 类型安全调用封装 + 错误归一化
 * 命令名与出入参定义见 contracts.ts（唯一事实源）；前端一律经本模块调用。
 */
import { invoke } from "@tauri-apps/api/core";
import type { IpcPayloads, IpcResults } from "./contracts";

/** 归一化 IPC 错误：Tauri 侧错误可能是任意字符串/对象 */
export class IpcError extends Error {
  constructor(command: string, detail: string) {
    super(`[${command}] ${detail}`);
    this.name = "IpcError";
  }
}

async function call<K extends keyof IpcPayloads & keyof IpcResults>(
  command: K,
  payload?: IpcPayloads[K]
): Promise<IpcResults[K]> {
  try {
    return await invoke<IpcResults[K]>(command, (payload ?? {}) as Record<string, unknown>);
  } catch (err) {
    const detail = typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
    throw new IpcError(command, detail);
  }
}

export const ipc = {
  settingsGet: (key?: string) => call("settings_get", { key }),
  settingsSet: (key: string, value: unknown) => call("settings_set", { key, value }),
  clipboardList: (limit?: number, pinnedOnly?: boolean) =>
    call("clipboard_list", { limit, pinnedOnly }),
  clipboardDelete: (id: string) => call("clipboard_delete", { id }),
  clipboardClear: () => call("clipboard_clear", {}),
  clipboardTogglePin: (id: string) => call("clipboard_toggle_pin", { id }),
  colorPickScreen: () => call("color_pick_screen", {}),
  windowToggle: () => call("window_toggle", {}),
  windowHide: () => call("window_hide", {}),
  openExternal: (url: string) => call("open_external", { url }),
};
