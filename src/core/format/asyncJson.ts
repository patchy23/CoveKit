import { formatJson, type FormatResult } from './json'

// 仅决定执行位置，不限制正文/结果；小文本避免 Worker 启动和跨线程传输成本。
export const JSON_WORKER_THRESHOLD = 64 * 1024

/** 每个消费者一项在途计算；新输入终止旧计算，完成或关闭后释放计算线程。 */
export function createJsonFormatter() {
  let worker: Worker | undefined
  let sequence = 0
  let pending: ((error: Error) => void) | undefined
  function destroy() {
    sequence++
    worker?.terminate()
    worker = undefined
    pending?.(new DOMException('格式化已取消', 'AbortError'))
    pending = undefined
  }
  function cancel() {
    if (pending) destroy()
  }
  function run(text: string, indent = 2): FormatResult | Promise<FormatResult> {
    cancel()
    if (text.length <= JSON_WORKER_THRESHOLD) return formatJson(text, indent)
    return new Promise((resolve, reject) => {
      try {
        worker ??= new Worker(new URL('./json.worker.ts', import.meta.url), { type: 'module' })
        const current = worker
        const id = ++sequence
        pending = reject
        worker.onmessage = (event: MessageEvent<{ id: number; result: FormatResult }>) => {
          if (worker !== current || sequence !== id || event.data.id !== id) return
          pending = undefined
          resolve(event.data.result)
          destroy()
        }
        worker.onerror = (event) => {
          if (worker !== current || sequence !== id) return
          pending = undefined
          reject(new Error(event.message || 'JSON 格式化工作器运行失败'))
          destroy()
        }
        worker.postMessage({ id, text, indent })
      } catch (error) {
        pending = undefined
        reject(error)
        destroy()
      }
    })
  }
  return { run, cancel, destroy }
}
