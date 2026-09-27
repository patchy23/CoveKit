import { isValidJson } from '@/core/format/json'
import { createWorkerTask } from '@/core/format/workerTask'

// 只切换超大正文的校验位置，不限制请求大小；普通正文保持原同步路径。
export const REQUEST_JSON_WORKER_THRESHOLD = 8 * 1024 * 1024

/** 少量长字符串的解析成本较低；用原生字符串搜索跳过正文，避免无收益的线程往返。 */
export function needsJsonWorker(text: string): boolean {
  if (text.length <= REQUEST_JSON_WORKER_THRESHOLD) return false
  if (text.length >= 32 * 1024 * 1024) return true
  const token = /[{}[\],:"]/g
  let operations = 0
  let match: RegExpExecArray | null
  while ((match = token.exec(text))) {
    if (++operations >= 256) return true
    if (match[0] !== '"') continue
    let end = match.index
    let closed = false
    do {
      end = text.indexOf('"', end + 1)
      if (end < 0) return false // 是否合法仍由 JSON.parse 判断。
      if (++operations >= 256) return true
      let escapes = 0
      for (let i = end - 1; i >= 0 && text[i] === '\\'; i--) {
        if (++operations >= 256) return true
        escapes++
      }
      closed = escapes % 2 === 0
    } while (!closed)
    token.lastIndex = end + 1
  }
  return false
}

/** 请求页签独占校验任务，取消后不得继续发送尚未通过校验的请求。 */
export function createRequestValidator() {
  const task = createWorkerTask<{ text: string }, boolean>(
    () => new Worker(new URL('./requestValidation.worker.ts', import.meta.url), { type: 'module' }),
    '请求体校验工作器运行失败',
    60_000
  )
  function run(text: string): boolean | Promise<boolean> {
    task.cancel()
    return needsJsonWorker(text) ? task.run({ text }) : isValidJson(text)
  }
  return { run, cancel: task.destroy, destroy: task.destroy }
}
