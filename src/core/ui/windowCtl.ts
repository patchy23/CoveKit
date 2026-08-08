/**
 * 窗口控制公共函数（窗口按钮组件共用）
 * 非 Tauri 环境（浏览器预览）惰性获取窗口句柄，取不到则窗口操作静默跳过。
 */
import { getCurrentWindow, type Window } from "@tauri-apps/api/window";

const isTauri = "__TAURI_INTERNALS__" in window;

/** 惰性获取当前窗口句柄（非 Tauri 环境返回 null） */
export function windowCtl(): Window | null {
  if (!isTauri) return null;
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

/** 安全执行窗口操作：取不到句柄或操作失败都静默告警，不抛错 */
export function safeWindow(fn: (w: Window) => Promise<unknown>) {
  const w = windowCtl();
  if (!w) return;
  fn(w).catch((e) => console.warn("[window] 窗口操作失败（检查 capabilities 权限）:", e));
}
