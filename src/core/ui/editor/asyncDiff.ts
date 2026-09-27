import { createWorkerTask } from '../../format/workerTask'
import { computeDiff, type DiffRequest, type DiffResult } from './diffComputation'
export { restoreChunks, type DiffResult } from './diffComputation'

export const DIFF_WORKER_THRESHOLD = 256 * 1024
export function createDiffTask() {
  const task = createWorkerTask<DiffRequest, DiffResult>(
    () => new Worker(new URL('./diff.worker.ts', import.meta.url), { type: 'module' }),
    '差异计算工作器运行失败',
    60_000
  )
  return {
    run(request: DiffRequest): DiffResult | Promise<DiffResult> {
      task.cancel()
      if (request.original === request.modified)
        return {
          versionA: request.versionA,
          versionB: request.versionB,
          chunks: [],
          stats: { added: 0, removed: 0, same: true },
        }
      return request.original.length + request.modified.length < DIFF_WORKER_THRESHOLD
        ? computeDiff(request)
        : task.run(request)
    },
    cancel: task.cancel,
    destroy: task.destroy,
  }
}
