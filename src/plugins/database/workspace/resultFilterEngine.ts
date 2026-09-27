export type FilterMessage =
  | { type: 'begin'; version: number; length: number; reset: boolean }
  | {
      type: 'rows'
      version: number
      batch: number
      changes: Array<{ index: number; row: string[] }>
    }
  | { type: 'query'; version: number; id: number; term: string }
  | { type: 'cancel'; version: number }

/** 同一 Worker 只保留当前数据及最新查询；逐片让出执行权以接收替换和取消。 */
export function createFilterEngine(
  emit: (data: object, transfer?: Transferable[]) => void,
  yieldTask = () =>
    new Promise<void>((resolve) => {
      // 消息任务让出执行权但不受后台页面定时器最小间隔影响；每次完成关闭端口。
      const channel = new MessageChannel()
      channel.port1.onmessage = () => {
        channel.port1.close()
        channel.port2.close()
        resolve()
      }
      channel.port2.postMessage(null)
    })
) {
  let rows: string[][] = []
  let version = 0
  let latest: Extract<FilterMessage, { type: 'query' }> | undefined
  let running = false
  async function compute() {
    if (running) return
    running = true
    try {
      while (latest) {
        const query = latest
        const matches: number[] = []
        const literal = /^[\x20-\x40\x5b-\x60\x7b-\x7e]+$/.test(query.term)
        let deadline = performance.now() + 8
        for (let index = 0; index < rows.length; index++) {
          if (
            rows[index].some((cell) => (literal ? cell : cell.toLowerCase()).includes(query.term))
          )
            matches.push(index)
          if (performance.now() >= deadline) {
            await yieldTask()
            if (latest !== query || version !== query.version) break
            deadline = performance.now() + 8
          }
        }
        if (latest !== query || version !== query.version) continue
        latest = undefined
        const indices = Uint32Array.from(matches)
        emit({ type: 'result', version, id: query.id, indices }, [indices.buffer])
      }
    } catch {
      if (latest) emit({ type: 'error', version, id: latest.id })
      latest = undefined
    } finally {
      running = false
    }
  }
  function receive(data: FilterMessage) {
    if (data.type === 'begin') {
      latest = undefined
      version = data.version
      if (data.reset) rows = new Array(data.length)
      else rows.length = data.length
      emit({ type: 'ready', version })
    } else if (data.version === version) {
      if (data.type === 'rows') {
        for (const { index, row } of data.changes) rows[index] = row
        emit({ type: 'rows', version, batch: data.batch })
      } else if (data.type === 'cancel') latest = undefined
      else {
        latest = data
        void compute()
      }
    }
  }
  return { receive }
}
