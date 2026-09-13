/**
 * 数据库工具的资源生命周期（可靠性 T10-4）
 *
 * 关闭策略（本插件自定）：**关闭工具页签会断开全部数据库连接**（连接池随页签释放）。
 * - 不弹询问：断开数据库连接没有数据丢失风险，重启工具即可重连，拦一下反而啰嗦；
 * - 执行中的查询由用户在页签内自行取消（`dbc_cancel`），这里只负责把连接收干净；
 * - 退出应用走同一套清理，不把连接泄漏到进程结束。
 *
 * 页签内关闭单个连接页签（query workspace）是插件内部行为，与本模块无关。
 */
import { connectionIpc } from './ipc'
import { useToolLifecycle } from '@/core/lifecycle'

export function useDatabaseToolLifecycle() {
  useToolLifecycle('database', {
    owner: 'database.connections',
    dispose: async () => {
      const connections = await connectionIpc.list()
      const failures: string[] = []
      for (const connection of connections) {
        try {
          await connectionIpc.disconnect(connection.id)
        } catch (error) {
          failures.push(
            `${connection.label}：${error instanceof Error ? error.message : String(error)}`
          )
        }
      }
      if (failures.length > 0) throw new Error(`断开失败（${failures.join('；')}）`)
    },
  })
}
