import { formatJson, type FormatResult } from './json'
import { createWorkerTask } from './workerTask'

// 仅决定执行位置，不限制正文/结果；小文本避免 Worker 启动和跨线程传输成本。
export const JSON_WORKER_THRESHOLD = 64 * 1024

/** 每个消费者一项在途计算；新输入终止旧计算，完成或关闭后释放计算线程。 */
export function createJsonFormatter() {
  const task = createWorkerTask<{ text: string; indent: number }, FormatResult>(
    () => new Worker(new URL('./json.worker.ts', import.meta.url), { type: 'module' }),
    'JSON 格式化工作器运行失败'
  )
  function run(text: string, indent = 2): FormatResult | Promise<FormatResult> {
    task.cancel()
    if (text.length <= JSON_WORKER_THRESHOLD) return formatJson(text, indent)
    return task.run({ text, indent })
  }
  return { run, cancel: task.cancel, destroy: task.destroy }
}
