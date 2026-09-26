import { createWorkerTask } from '@/core/format/workerTask'
import { decodeBase64, encodeBase64, type Base64Result } from './useBase64'

export function createBase64Task() {
  const task = createWorkerTask<{ text: string; direction: 'encode' | 'decode' }, Base64Result>(
    () => new Worker(new URL('./base64.worker.ts', import.meta.url), { type: 'module' }),
    'Base64 转换工作器运行失败'
  )
  function run(text: string, direction: 'encode' | 'decode'): Base64Result | Promise<Base64Result> {
    task.cancel()
    // 调度阈值而非内容上限，小输入保留直接处理的即时反馈。
    if (text.length <= 64 * 1024)
      return direction === 'encode' ? encodeBase64(text) : decodeBase64(text)
    return task.run({ text, direction })
  }
  return { run, cancel: task.cancel, destroy: task.destroy }
}
