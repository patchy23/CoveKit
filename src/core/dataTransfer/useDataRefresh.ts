/** 导入后刷新持久列表；订阅由当前组件或 store 作用域释放。 */
import { onScopeDispose } from 'vue'
import { onSpaceDataChanged } from '@/core/ipc/spaceEvents'
import { useUiStore } from '@/stores/ui'

export function useDataRefresh(prefix: string, refresh: () => void | Promise<unknown>): void {
  const ui = useUiStore()
  const dispose = onSpaceDataChanged((datasets) => {
    if (!datasets.includes('*') && !datasets.some((dataset) => dataset.startsWith(prefix))) return
    void Promise.resolve()
      .then(refresh)
      .catch((error: unknown) => {
        ui.toast(`导入后刷新失败：${error instanceof Error ? error.message : String(error)}`)
      })
  })
  onScopeDispose(dispose)
}
