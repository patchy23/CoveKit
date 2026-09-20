/**
 * database 查询历史与收藏域：只负责历史/收藏的读取、保存与增删，
 * 不拥有查询执行生命周期（执行与结果状态在 workspace 域），
 * 因此 workspace 每完成一次执行只是经 recordHistory 端口把结果落库。
 */
import { ref } from 'vue'
import type { ExecutionScope, HistoryEntry, SavedEntry } from '../contracts'
import { historyIpc, savedIpc } from '../ipc'

/** library 域的跨域协作端口（由根门面注入） */
export interface QueryLibraryPorts {
  /** 应用级错误提示（门面持有 errorHint） */
  showError: (err: unknown) => void
}

export function useQueryLibrary(ports: QueryLibraryPorts) {
  const history = ref<HistoryEntry[]>([])
  const savedSql = ref<SavedEntry[]>([])

  async function refreshHistory() {
    try {
      history.value = await historyIpc.list()
    } catch (err) {
      ports.showError(err)
    }
  }

  async function refreshSaved() {
    try {
      savedSql.value = await savedIpc.list()
    } catch (err) {
      ports.showError(err)
    }
  }

  /** 记录一次查询执行并刷新历史列表（执行生命周期归 workspace 域，本域只落库与读取） */
  function recordHistory(
    connId: string,
    sql: string,
    status: string,
    durationMs: number,
    scope?: ExecutionScope
  ) {
    void historyIpc
      .add(connId, sql, status, durationMs, scope)
      .then(refreshHistory)
      .catch(ports.showError)
  }

  /** 新增收藏，返回收藏 id */
  async function addSaved(title: string, sql: string): Promise<number> {
    return savedIpc.add(title, sql)
  }

  /** 更新收藏（Ctrl+S 保存与页签别名同步） */
  async function updateSaved(id: number, title: string, sql: string): Promise<void> {
    await savedIpc.update(id, title, sql)
  }

  async function removeSaved(id: number) {
    await savedIpc.remove(id)
    await refreshSaved()
  }

  async function clearHistory() {
    await historyIpc.clear()
    await refreshHistory()
  }

  return {
    history,
    savedSql,
    refreshHistory,
    refreshSaved,
    recordHistory,
    addSaved,
    updateSaved,
    removeSaved,
    clearHistory,
  }
}
