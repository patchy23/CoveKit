import { formatSql } from './sql'
import { createWorkerTask } from './workerTask'

// 调度阈值只选择执行位置，不限制 SQL 或格式化结果的大小。
export const SQL_WORKER_THRESHOLD = 64 * 1024
export function createSqlFormatter() {
  const task = createWorkerTask<{ text: string }, string>(
    () => new Worker(new URL('./sql.worker.ts', import.meta.url), { type: 'module' }),
    'SQL 格式化工作器运行失败'
  )
  function run(text: string): string | Promise<string> {
    task.cancel()
    return text.length <= SQL_WORKER_THRESHOLD ? formatSql(text) : task.run({ text })
  }
  return { run, cancel: task.cancel, destroy: task.destroy }
}
