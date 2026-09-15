/**
 * 框架 · 应用退出协商（可靠性 T10-3、AR06 §9.2）
 *
 * 退出只有一条路：**先问页面 owner → 与后端模块 blockers 合成一次裁决 → 允许才清理 → 提交退出**；
 * 被业务拒绝时后端把原因回传，界面必须给出三个明确出路（重试 / 强制退出 / 取消），
 * 而不是「点了没反应」。
 *
 * 不变式：
 * - 前端不直接调 `app.exit` 之类的原生能力：一律经 IPC，保证与托盘退出同一套语义；
 * - 强制退出是用户显式动作，界面必须同时说明「清理可能不完整」；
 * - **裁决通过之前不清理**：原来先 `disposeAllTools` 再请后端裁决，用户点「取消」时前端已经清完了；
 * - 后端清理仍只在 `RunEvent::Exit` 一处执行，提交命令不重复清理。
 */
import { invokeCommand } from '@/core/ipc/ipc'
import type { CloseDecision } from '@/core/ipc/contracts'
import { commitBackendClose, requestBackendClose } from './closeBridge'
import { collectAllToolBlockers, disposeAllTools } from './toolContext'
import type { CloseReason } from './types'

export type { CloseDecision }

/** 后端上报拒绝关闭的事件名（与 Rust `exit::EXIT_VETO_EVENT` 一致） */
export const EXIT_VETO_EVENT = 'app://exit-vetoed'

/** IPC 失败的收口：一律当成「没通过」，界面按拒绝展示原因，用户可以重试或强退 */
function failureDecision(message: string): CloseDecision {
  return { proceed: false, forced: false, blockers: [message], failures: [] }
}

/**
 * 请求退出应用。
 *
 * 顺序：问页面内 owner（未保存内容/运行中任务）→ 后端合成裁决 → 通过才清理页面内 owner 并提交退出。
 *
 * @param reason 关闭原因（默认 exit；更新安装用 update、重启用 restart）
 */
export async function requestAppExit(reason: CloseReason = 'exit'): Promise<CloseDecision> {
  const blockers = await collectAllToolBlockers(reason)
  let decision: CloseDecision
  try {
    decision = await requestBackendClose(reason, null, blockers, false)
  } catch (error) {
    return failureDecision(
      `退出前询问后端失败：${error instanceof Error ? error.message : String(error)}`
    )
  }
  if (!decision.proceed) return decision

  const cleanup = await disposeAllTools(reason)
  if (cleanup.failures.length > 0) {
    // 清理失败不阻止退出，但必须留痕（用户可在诊断里看到）
    console.warn('[app-close] 退出前清理失败:', cleanup.failures)
  }
  try {
    return await commitBackendClose(reason, null, false)
  } catch (error) {
    return failureDecision(
      `提交退出失败：${error instanceof Error ? error.message : String(error)}`
    )
  }
}

/** 用户显式强制退出：跳过业务拦截，清理仍有总超时（可能来不及清完） */
export async function forceAppExit(): Promise<void> {
  await invokeCommand<Record<string, never>, CloseDecision>('app_force_exit')
}

/**
 * 订阅后端的「关闭被拒绝」事件。
 *
 * 托盘菜单退出时窗口处于隐藏状态，后端会把窗口唤到前台并发这条事件，
 * 界面据此弹提示，用户才能真正决定下一步。
 *
 * @param handler 收到拒绝原因（含 owner 前缀）
 * @returns 取消订阅函数（组件卸载时调用）
 */
export async function watchExitVeto(
  handler: (decision: CloseDecision) => void
): Promise<() => void> {
  try {
    // 动态导入：浏览器预览环境没有 Tauri 事件通道，这里不应让整个界面起不来
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<CloseDecision>(EXIT_VETO_EVENT, (event) => handler(event.payload))
  } catch (error) {
    console.warn('[app-close] 订阅退出拒绝事件失败:', error)
    return () => {}
  }
}
