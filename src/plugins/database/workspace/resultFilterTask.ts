// 仅用于调度，不限制结果、行或单元格；单个大行完整交给线程处理。
const BATCH_UNITS = 256 * 1024
const BATCH_ROWS = 4096

export function needsAsyncFilter(rows: string[][]): boolean {
  let units = 0
  for (const row of rows) {
    units += row.length + 1
    for (const cell of row) units += cell.length
    if (units > BATCH_UNITS) return true
  }
  return false
}

/** 每次筛选一个线程、一个在途批次；完成/失败/取消释放线程及快照闭包。 */
export function createResultFilterTask() {
  let worker: Worker | undefined
  let rejectPending: ((error: Error) => void) | undefined
  function destroy() {
    if (worker) {
      worker.onmessage = null
      worker.onerror = null
      worker.onmessageerror = null
      worker.terminate()
    }
    worker = undefined
    rejectPending?.(new DOMException('筛选已取消', 'AbortError'))
    rejectPending = undefined
  }
  function run(rows: string[][], term: string): Promise<string[][]> {
    destroy()
    if (!rows.length) return Promise.resolve([])
    return new Promise((resolve, reject) => {
      rejectPending = reject
      try {
        const current = new Worker(new URL('./resultFilter.worker.ts', import.meta.url), {
          type: 'module',
        })
        worker = current
        let start = 0
        let end = 0
        let id = 0
        const matches: string[][] = []
        const fail = (error: Error) => {
          if (worker !== current) return
          rejectPending = undefined
          reject(error)
          destroy()
        }
        const send = () => {
          if (end === rows.length) {
            rejectPending = undefined
            resolve(matches)
            destroy()
            return
          }
          start = end
          let units = 0
          while (end < rows.length && end - start < BATCH_ROWS && units < BATCH_UNITS) {
            const row = rows[end++]
            units += row.length + 1
            for (const cell of row) units += cell.length
          }
          current.postMessage({ id: ++id, rows: rows.slice(start, end), term })
        }
        current.onmessage = (event: MessageEvent<unknown>) => {
          if (worker !== current) return
          try {
            const data = event.data as { id?: number; matches?: unknown }
            if (!data || data.id !== id || !Array.isArray(data.matches))
              throw new Error('筛选工作器响应格式无效')
            let previous = -1
            for (const index of data.matches) {
              if (!Number.isInteger(index) || index <= previous || index >= end - start)
                throw new Error('筛选工作器行位置无效')
              matches.push(rows[start + index])
              previous = index
            }
            send()
          } catch (error) {
            fail(error instanceof Error ? error : new Error('筛选工作器运行失败'))
          }
        }
        current.onerror = () => fail(new Error('筛选工作器运行失败'))
        current.onmessageerror = () => fail(new Error('无法读取筛选结果'))
        send()
      } catch (error) {
        rejectPending = undefined
        reject(error)
        destroy()
      }
    })
  }
  return { run, destroy }
}
