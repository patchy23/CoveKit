import { onScopeDispose, ref } from 'vue'
import { ipc } from './ipc'

/** 每个列表/查询页独立取消，登记迟到、旧响应和关闭都不会写回新页面。 */
export function useDnsRead(onCleanupError: (error: unknown) => void) {
  const busy = ref(false)
  let sequence = 0
  let disposed = false
  type Lease = { id: string; released: boolean }
  let active: Lease | undefined
  function release(lease: Lease) {
    if (lease.released) return
    lease.released = true
    void ipc.dnsReadCancel(lease.id).catch(onCleanupError)
  }
  function cancel() {
    sequence++
    busy.value = false
    if (active) release(active)
    active = undefined
  }
  async function run<T>(read: (id: string) => Promise<T>): Promise<T | undefined> {
    if (disposed) return
    cancel()
    const version = sequence
    busy.value = true
    let lease: Lease | undefined
    try {
      lease = { id: await ipc.dnsReadPrepare(), released: false }
      if (disposed || version !== sequence) return
      active = lease
      const result = await read(lease.id)
      if (!disposed && version === sequence) return result
    } catch (error) {
      if (!disposed && version === sequence) throw error
    } finally {
      // 正常结束后后端已清理；仍发幂等释放以覆盖只完成登记、尚未发送读命令的路径。
      if (lease) release(lease)
      if (version === sequence) {
        active = undefined
        busy.value = false
      }
    }
  }
  onScopeDispose(() => {
    disposed = true
    cancel()
  })
  return { busy, run, cancel }
}
