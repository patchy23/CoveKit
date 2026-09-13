/**
 * 框架 · 应用退出协商（可靠性 T10-3）
 *
 * 退出只有一条路：请求 → 后端 prepare → 允许才退出；被业务拒绝时后端把原因回传，
 * 界面必须给出三个明确出路（重试 / 强制退出 / 取消），而不是「点了没反应」。
 *
 * 不变式：
 * - 前端不直接调 `app.exit` 之类的原生能力：一律经 IPC，保证与托盘退出同一套语义；
 * - 强制退出是用户显式动作，界面必须同时说明「清理可能不完整」；
 * - 退出前先跑本进程内的工具清理（disposeAllTools），再请后端退出。
 */
import { invokeCommand } from '@/core/ipc/ipc'
import type { ExitDecision } from '@/core/ipc/contracts'
import { disposeAllTools } from './toolContext'
import type { CloseReason } from './types'

export type { ExitDecision }

/** 后端上报拒绝退出的事件名（与 Rust `exit::EXIT_VETO_EVENT` 一致） */
export const EXIT_VETO_EVENT = 'app://exit-vetoed'

/** 退出请求参数（扁平 `reason`，与后端 `ExitRequest` 对齐） */
interface ExitPayload {
  reason: CloseReason
}

/**
 * 请求退出应用。
 *
 * 先做前端侧清理（各工具的 dispose），再请后端走 prepare；被拒绝时原样返回原因。
 *
 * @param reason 关闭原因（默认 exit；更新安装用 update、重启用 restart）
 */
export async function requestAppExit(reason: CloseReason = 'exit'): Promise<ExitDecision> {
  const cleanup = await disposeAllTools(reason)
  if (cleanup.failures.length > 0) {
    // 清理失败不阻止退出，但必须留痕（用户可在诊断里看到）
    console.warn('[app-close] 退出前清理失败:', cleanup.failures)
  }
  return invokeCommand<ExitPayload, ExitDecision>('app_request_exit', { reason })
}

/** 用户显式强制退出：跳过业务拦截，清理仍有总超时（可能来不及清完） */
export async function forceAppExit(): Promise<void> {
  await invokeCommand<Record<string, never>, ExitDecision>('app_force_exit')
}

/**
 * 订阅后端的「退出被拒绝」事件。
 *
 * 托盘菜单退出时窗口处于隐藏状态，后端会把窗口唤到前台并发这条事件，
 * 界面据此弹提示，用户才能真正决定下一步。
 *
 * @param handler 收到拒绝原因（含 owner 前缀）
 * @returns 取消订阅函数（组件卸载时调用）
 */
export async function watchExitVeto(
  handler: (decision: ExitDecision) => void
): Promise<() => void> {
  try {
    // 动态导入：浏览器预览环境没有 Tauri 事件通道，这里不应让整个界面起不来
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<ExitDecision>(EXIT_VETO_EVENT, (event) => handler(event.payload))
  } catch (error) {
    console.warn('[app-close] 订阅退出拒绝事件失败:', error)
    return () => {}
  }
}
