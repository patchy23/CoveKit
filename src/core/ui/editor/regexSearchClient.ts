import type { Text } from '@codemirror/state'
import type { RegexRequest, RegexResponse } from './regexSearch'

/** 一个编辑器最多持有一个计算 Worker；条件/文档失效时直接终止不可协作中断的正则。 */
export function createRegexClient(readText: (document: Text) => string) {
  let worker: Worker | undefined
  let document: Text | undefined
  let sequence = 0
  let rejectPending: ((error: Error) => void) | undefined
  function reset() {
    worker?.terminate()
    worker = undefined
    document = undefined
    rejectPending?.(new DOMException('查找已取消', 'AbortError'))
    rejectPending = undefined
  }
  function run(doc: Text, request: Omit<RegexRequest, 'id' | 'document'>): Promise<RegexResponse> {
    return new Promise((resolve, reject) => {
      try {
        worker ??= new Worker(new URL('./regexSearch.worker.ts', import.meta.url), {
          type: 'module',
        })
        const current = worker
        const id = ++sequence
        rejectPending = reject
        worker.onmessage = (event: MessageEvent<RegexResponse>) => {
          if (worker !== current || sequence !== id || event.data.id !== id) return
          rejectPending = undefined
          resolve(event.data)
        }
        worker.onerror = (event) => {
          if (worker !== current || sequence !== id) return
          rejectPending = undefined
          reject(new Error(event.message || '正则查找工作器运行失败'))
          reset()
        }
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
