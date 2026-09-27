import { getCurrentScope, onScopeDispose, ref } from 'vue'
import { Channel } from '@tauri-apps/api/core'
import { ipc } from '@/core/ipc/ipc'
import type { TransferProgress } from '@/core/ipc/contracts'

/** 一个备份弹窗拥有一个活动请求；凭证正文和密码只由本次 operation 闭包持有。 */
export function useBackupRequest(report: (error: unknown) => void) {
  const busy = ref(false)
  const waitingMemory = ref(false)
  let current: { id?: string; cancelled: boolean } | undefined
  async function cancel() {
    if (!current) return
    current.cancelled = true
    waitingMemory.value = false
    if (current.id) await ipc.dataTransferCancel(current.id)
  }
  async function run<T>(
    operation: (id: string, progress: Channel<TransferProgress>) => Promise<T>
  ) {
    if (current) throw new Error('当前备份操作尚未完成')
    const request = { id: undefined as string | undefined, cancelled: false }
    current = request
    busy.value = true
    waitingMemory.value = false
    let progress: Channel<TransferProgress> | undefined
    try {
      request.id = await ipc.dataTransferPrepare()
      if (request.cancelled) throw new DOMException('已取消', 'AbortError')
      progress = new Channel<TransferProgress>()
      progress.onmessage = (event) => {
        if (current === request && !request.cancelled && event.requestId === request.id)
          waitingMemory.value = event.waitingMemory
      }
      return await operation(request.id, progress)
    } catch (error) {
      if (request.cancelled && String(error).includes('已取消'))
        throw new DOMException('已取消', 'AbortError')
      throw error
    } finally {
      if (progress) progress.onmessage = () => {}
      if (request.id) {
        try {
          await ipc.dataTransferCancel(request.id)
        } catch (error) {
          report(error)
        }
      }
      if (current === request) {
        current = undefined
        busy.value = false
        waitingMemory.value = false
      }
    }
  }
  if (getCurrentScope())
    onScopeDispose(() => {
      void cancel().catch(report)
    })
  return { busy, waitingMemory, run, cancel }
}
