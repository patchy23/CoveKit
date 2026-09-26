import { onScopeDispose } from 'vue'
import { ipc } from '../ipc'

/** 页面只拥有自己的扫描；切换、关闭和登记迟到均释放，不留整目录后台扫描。 */
export function useLocalDirectory(onCleanupError: (error: unknown) => void) {
  let sequence = 0
  let disposed = false
  type Lease = { id: string; released: boolean }
  let active: Lease | undefined
  function release(lease: Lease) {
    if (lease.released) return
    lease.released = true
    void ipc.sshLocalListCancel(lease.id).catch(onCleanupError)
  }
  function cancel() {
    sequence++
    if (active) release(active)
    active = undefined
  }
  async function read(path: string) {
    if (disposed) return
    cancel()
    const version = sequence
    let lease: Lease | undefined
    try {
      lease = { id: await ipc.sshLocalListPrepare(), released: false }
      if (disposed || version !== sequence) return
      active = lease
      const result = await ipc.sshLocalList(path, lease.id)
      if (!disposed && version === sequence) return result
    } catch (error) {
      if (!disposed && version === sequence) throw error
    } finally {
      if (lease) release(lease)
      if (version === sequence) active = undefined
    }
  }
  onScopeDispose(() => {
    disposed = true
    cancel()
  })
  return { read }
}
