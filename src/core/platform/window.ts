/**
 * 平台能力 · 窗口控制
 *
 * 基础 UI 不再持有窗口句柄：这里把窗口操作收敛为具名动作并返回明确结果，
 * 调用方（业务界面）按结果决定是否给反馈。非桌面环境（浏览器预览）返回
 * `unsupported`，属预期缺失而非失败。
 */
import { getCurrentWindow, type Window } from '@tauri-apps/api/window'

/** 窗口动作结果：`unsupported` = 非桌面运行环境；`failed` = 平台调用失败（权限/句柄异常） */
export type WindowActionOutcome =
  | { ok: true }
  | { ok: false; reason: 'unsupported' }
  | { ok: false; reason: 'failed'; error: unknown }

/** 支持的窗口动作（新增动作在此登记，界面不直接接触窗口句柄） */
export type WindowAction = 'minimize' | 'toggleMaximize' | 'close'

/** 是否运行在桌面容器内（浏览器预览为 false） */
export function isDesktopRuntime(): boolean {
  return '__TAURI_INTERNALS__' in window
}

/** 惰性取当前窗口句柄；非桌面环境或取不到时返回 null */
function currentWindow(): Window | null {
  if (!isDesktopRuntime()) return null
  try {
    return getCurrentWindow()
  } catch {
    return null
  }
}

/**
 * 执行一次窗口动作。
 *
 * @param action 具名动作，见 `WindowAction`
 * @returns 结果对象；失败与不可用都不抛错，由调用方适配提示
 */
export async function runWindowAction(action: WindowAction): Promise<WindowActionOutcome> {
  const win = currentWindow()
  if (!win) return { ok: false, reason: 'unsupported' }
  try {
    if (action === 'minimize') await win.minimize()
    else if (action === 'toggleMaximize') await win.toggleMaximize()
    else await win.close()
    return { ok: true }
  } catch (error) {
    // 保留诊断（多为 capabilities 权限缺失），同时把结果交回调用方做可见反馈
    console.warn('[window] 窗口操作失败（检查 capabilities 权限）:', error)
    return { ok: false, reason: 'failed', error }
  }
}
