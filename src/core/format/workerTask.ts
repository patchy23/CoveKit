/** 每个消费者最多一个在途任务；允许短期复用空闲 Worker，取消和销毁立即释放。 */
export function createWorkerTask<Request extends object, Result>(
  create: () => Worker,
  failure: string,
  idleTimeoutMs = 0
) {
  let worker: Worker | undefined
  let sequence = 0
  let pending: ((error: Error) => void) | undefined
  let idleTimer: ReturnType<typeof setTimeout> | undefined
  function destroy() {
    clearTimeout(idleTimer)
    idleTimer = undefined
    sequence++
    if (worker) {
      worker.onmessage = null
      worker.onerror = null
      worker.onmessageerror = null
      worker.terminate()
    }
    worker = undefined
    pending?.(new DOMException('计算已取消', 'AbortError'))
    pending = undefined
  }
  function cancel() {
    if (pending) destroy()
  }
  function run(request: Request): Promise<Result> {
    cancel()
    return new Promise((resolve, reject) => {
      try {
        clearTimeout(idleTimer)
        idleTimer = undefined
        worker ??= create()
        const current = worker
        const id = ++sequence
        pending = reject
        const fail = (message: string) => {
          if (worker !== current || sequence !== id) return
          pending = undefined
          reject(new Error(message))
          destroy()
        }
        worker.onmessage = (event: MessageEvent<unknown>) => {
          if (worker !== current || sequence !== id) return
          const data = event.data
          if (
            !data ||
            typeof data !== 'object' ||
            !('id' in data) ||
            !Number.isSafeInteger(data.id)
          ) {
            fail(`${failure}：响应格式无效`)
            return
          }
          if (data.id !== id) return
          if (!('result' in data)) {
            fail(`${failure}：响应缺少计算结果`)
            return
          }
          pending = undefined
          resolve(data.result as Result)
          current.onmessage = null
          current.onerror = null
          current.onmessageerror = null
          if (idleTimeoutMs > 0) idleTimer = setTimeout(destroy, idleTimeoutMs)
          else destroy()
        }
        worker.onerror = (event) => fail(event.message || failure)
        worker.onmessageerror = () => fail(`${failure}：无法读取计算结果`)
        worker.postMessage({ ...request, id })
      } catch (error) {
        pending = undefined
        reject(error)
        destroy()
      }
    })
  }
  return { run, cancel, destroy }
}
