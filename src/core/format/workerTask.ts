/** 独立计算任务的归属：每个消费者最多一个在途 Worker，替换、完成、关闭均释放。 */
export function createWorkerTask<Request extends object, Result>(
  create: () => Worker,
  failure: string
) {
  let worker: Worker | undefined
  let sequence = 0
  let pending: ((error: Error) => void) | undefined
  function destroy() {
    sequence++
    worker?.terminate()
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
        worker = create()
        const current = worker
        const id = ++sequence
        pending = reject
        worker.onmessage = (event: MessageEvent<{ id: number; result: Result }>) => {
          if (worker !== current || sequence !== id || event.data.id !== id) return
          pending = undefined
          resolve(event.data.result)
          destroy()
        }
        worker.onerror = (event) => {
          if (worker !== current || sequence !== id) return
          pending = undefined
          reject(new Error(event.message || failure))
          destroy()
        }
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
