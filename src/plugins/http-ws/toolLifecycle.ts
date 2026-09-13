/**
 * 接口调试工具的资源生命周期（可靠性 T10-4）
 *
 * 关闭策略（本插件自定）：
 * - **关闭工具页签会关闭全部 WebSocket 会话**（长连接留着没人看，且占用服务端连接）；
 * - HTTP 请求是后端一次性执行、无可取消句柄：**不做假清理**，关闭后结果直接丢弃；
 *   请求本身在后端跑完就结束，不会留下需要回收的资源。
 *
 * 因此这里只清理真正持有资源的 WS 会话，失败逐条上报（不允许静默）。
 */
import { ipc } from './ipc'
import { useToolLifecycle } from '@/core/lifecycle'

export function useHttpWsToolLifecycle() {
  useToolLifecycle('http-ws', {
    owner: 'http-ws.websockets',
    dispose: async () => {
      const sessions = await ipc.wsSessions()
      const open = sessions.filter((item) => item.open)
      if (open.length === 0) return
      const failures: string[] = []
      for (const session of open) {
        try {
          const result = await ipc.wsClose(session.id)
          if (!result.ok) failures.push(`${session.url}：${result.message ?? '关闭失败'}`)
        } catch (error) {
          failures.push(`${session.url}：${error instanceof Error ? error.message : String(error)}`)
        }
      }
      if (failures.length > 0) throw new Error(`WebSocket 关闭失败（${failures.join('；')}）`)
    },
  })
}
