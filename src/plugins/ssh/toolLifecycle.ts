/**
 * SSH 工具的资源生命周期（可靠性 T10-4）
 *
 * 关闭策略（本插件自定，框架只协调）：
 * - 关闭工具页签**会断开连接**：会话留在后台没人管，比误断更危险；
 * - 仍有连接/连接中时先询问，用户确认后才断开；隧道随连接一起停止；
 * - 退出应用走同一套清理（不会留下挂着的远程会话）。
 *
 * 切换页签、窗口失焦、被设置页覆盖都**不**走这里：那些场景会话必须保持。
 */
import { ipc } from './ipc'
import { useToolLifecycle } from '@/core/lifecycle'

/** 仍在使用的连接状态（这些会话关闭页签时会被断开） */
const LIVE_STATUS = ['connected', 'connecting', 'reconnecting']

export function useSshToolLifecycle() {
  useToolLifecycle('ssh', {
    owner: 'ssh.sessions',
    prepare: async () => {
      const connections = await ipc.sshConnections()
      const live = connections.filter((item) => LIVE_STATUS.includes(item.status))
      if (live.length === 0) return null
      return `${live.length} 个连接仍在会话中，关闭后这些会话与隧道会被断开`
    },
    dispose: async () => {
      const connections = await ipc.sshConnections()
      const failures: string[] = []
      for (const connection of connections) {
        try {
          await ipc.sshDisconnect(connection.sessionId)
        } catch (error) {
          failures.push(
            `${connection.host ?? connection.sessionId}：${error instanceof Error ? error.message : String(error)}`
          )
        }
      }
      // 逐个收集后统一抛出：框架按 owner 记录失败并让用户看到，而不是断到一半静默结束
      if (failures.length > 0) throw new Error(`断开失败（${failures.join('；')}）`)
    },
  })
}
