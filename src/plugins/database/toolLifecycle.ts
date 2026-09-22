/**
 * 数据库工具的资源生命周期（可靠性 T10-4）
 *
 * 关闭策略（本插件自定）：**关闭工具页签会断开全部数据库连接**（连接池随页签释放）。
 * - 有未提交事务或执行中的查询时拒绝关闭，由用户先处理；
 * - 关闭前持久化 SQL 草稿，保存失败可见；
 * - 退出应用走同一套清理，不把连接泄漏到进程结束。
 *
 * 页签内关闭单个连接页签（query workspace）是插件内部行为，与本模块无关。
 */
import { hasGridChanges } from './workspace/useQueryWorkspace'
import type { QueryState } from './workspace/useQueryWorkspace'
import { connectionIpc } from './ipc'
import { useToolLifecycle } from '@/core/lifecycle'

export function useDatabaseToolLifecycle(workspace?: {
  queryStates: import('vue').Ref<
    Record<
      string,
      Pick<QueryState, 'transactionActive' | 'gridEdits' | 'gridSaving'> & { status: string }
    >
  >
  flushDrafts: () => Promise<void>
}) {
  useToolLifecycle('database', {
    owner: 'database.connections',
    prepare: async () => {
      const states = Object.values(workspace?.queryStates.value ?? {})
      if (states.some((state) => state.gridSaving || hasGridChanges(state)))
        return '结果有未保存修改或正在提交，请先保存或放弃修改。'
      if (states.some((state) => state.transactionActive))
        return '数据库有未提交事务，请先提交或回滚后关闭。'
      if (states.some((state) => state.status === 'running'))
        return '数据库查询仍在执行，请先取消或等待完成。'
      try {
        await workspace?.flushDrafts()
      } catch (error) {
        return (
          'SQL 草稿保存失败，请先另存文件：' +
          (error instanceof Error ? error.message : String(error))
        )
      }
      return null
    },
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
