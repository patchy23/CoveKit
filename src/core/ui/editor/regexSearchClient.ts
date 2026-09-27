import type { Text } from '@codemirror/state'
import type { RegexRequest, RegexResponse } from './regexSearch'

/** 一个编辑器最多持有一个计算 Worker；条件/文档失效时直接终止不可协作中断的正则。 */
export function createRegexClient(readText: (document: Text) => string) {
  let worker: Worker | undefined
  let document: Text | undefined
  let sequence = 0
  let rejectPending: ((error: Error) => void) | undefined
  function reset() {
    if (worker) {
      worker.onmessage = null
      worker.onerror = null
      worker.onmessageerror = null
      worker.terminate()
    }
    worker = undefined
    document = undefined
    rejectPending?.(new DOMException('查找已取消', 'AbortError'))
    rejectPending = undefined
  }
  function run(doc: Text, request: Omit<RegexRequest, 'id' | 'document'>): Promise<RegexResponse> {
    // 调用方通常先取消旧修订；客户端自身也不能覆盖仍在等待的 Promise。
    if (rejectPending) reset()
    return new Promise((resolve, reject) => {
      try {
        worker ??= new Worker(new URL('./regexSearch.worker.ts', import.meta.url), {
          type: 'module',
        })
        const current = worker
        const id = ++sequence
        rejectPending = reject
        const fail = (message: string) => {
          if (worker !== current || sequence !== id) return
          rejectPending = undefined
          reject(new Error(message))
          reset()
        }
        worker.onmessage = (event: MessageEvent<RegexResponse>) => {
          if (worker !== current || sequence !== id) return
          if (!event.data || !Number.isSafeInteger(event.data.id)) {
            fail('正则查找工作器响应格式无效')
            return
          }
          if (event.data.id !== id) return
          if (!event.data.scan || !Array.isArray(event.data.highlights)) {
            fail('正则查找工作器响应缺少计算结果')
            return
          }
          rejectPending = undefined
          resolve(event.data)
        }
        worker.onerror = (event) => fail(event.message || '正则查找工作器运行失败')
        worker.onmessageerror = () => fail('无法读取正则查找结果')
        worker.postMessage({
          ...request,
          id,
          ...(document !== doc ? { document: readText(doc) } : {}),
        })
        document = doc
      } catch (error) {
        reject(error)
        reset()
      }
    })
  }
  return { run, reset }
}
