import type { EditorEnvelope } from './protocol'
type Handler = (type: string, value: unknown) => unknown | Promise<unknown>
/** 定向、带回执的内存通信；关闭时拒绝等待者，不把草稿落到 localStorage。 */
export function createEditorBridge(
  send: (message: EditorEnvelope) => Promise<void>,
  handle: Handler,
  timeout = 15000
) {
  const waiting = new Map<
    string,
    {
      resolve: (value: unknown) => void
      reject: (error: Error) => void
      timer: ReturnType<typeof setTimeout>
    }
  >()
  let disposed = false
  async function receive(message: EditorEnvelope) {
    if (disposed) return
    if (message.reply) {
      const pending = waiting.get(message.id)
      if (!pending) return
      waiting.delete(message.id)
      clearTimeout(pending.timer)
      if (message.error) pending.reject(new Error(message.error))
      else pending.resolve(message.value)
      return
    }
    let value: unknown, error: string | undefined
    try {
      value = await handle(message.type, message.value)
    } catch (e) {
      error = String(e)
    }
    if (!disposed) await send({ id: message.id, type: message.type, reply: true, value, error })
  }
  function request<T = unknown>(type: string, value?: unknown): Promise<T> {
    if (disposed) return Promise.reject(new Error('编辑窗口通信已关闭'))
    const id = crypto.randomUUID()
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        waiting.delete(id)
        reject(new Error('编辑窗口响应超时，草稿仍保留在当前窗口'))
      }, timeout)
      waiting.set(id, { resolve: (result) => resolve(result as T), reject, timer })
      void send({ id, type, value }).catch((error) => {
        const pending = waiting.get(id)
        if (!pending) return
        waiting.delete(id)
        clearTimeout(timer)
        reject(error)
      })
    })
  }
  function dispose() {
    disposed = true
    for (const pending of waiting.values()) {
      clearTimeout(pending.timer)
      pending.reject(new Error('编辑窗口通信已关闭'))
    }
    waiting.clear()
  }
  return { receive, request, dispose }
}
