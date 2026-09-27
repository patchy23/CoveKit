const BATCH_UNITS = 256 * 1024
const BATCH_ROWS = 4096
export const FILTER_WORKER_UNITS = 8 * 1024 * 1024

/** 只决定执行位置，不限制数据规模；原始查询快照只计算一次规模。 */
export function needsAsyncFilter(rows: string[][]): boolean {
  let units = 0
  for (const row of rows) {
    units += row.length + 1
    for (const cell of row) units += cell.length
    if (units >= FILTER_WORKER_UNITS) return true
  }
  return false
}

type Search = {
  id: number
  term: string
  resolve: (indices: Uint32Array) => void
  reject: (error: Error) => void
}
type Upload = { previous?: string[][]; rows: string[][]; next: number; batch: number }

/** 一个结果快照、一个在途传输批次和一个最新搜索；搜索之间复用后台数据。 */
export function createResultFilterTask() {
  let worker: Worker | undefined
  let snapshot: string[][] | undefined
  let version = 0
  let sequence = 0
  let upload: Upload | undefined
  let pending: Search | undefined
  let idle: ReturnType<typeof setTimeout> | undefined
  function abortError() {
    return new DOMException('筛选已取消', 'AbortError')
  }
  function destroy(error: Error = abortError()) {
    clearTimeout(idle)
    if (worker) {
      worker.onmessage = null
      worker.onerror = null
      worker.onmessageerror = null
      worker.terminate()
    }
    worker = undefined
    snapshot = undefined
    upload = undefined
    pending?.reject(error)
    pending = undefined
  }
  function park() {
    clearTimeout(idle)
    idle = setTimeout(destroy, 60_000)
  }
  function sendQuery() {
    if (!pending) {
      park()
      return
    }
    worker!.postMessage({ type: 'query', version, id: pending.id, term: pending.term })
  }
  function sendBatch() {
    if (!upload) return
    const changes: Array<{ index: number; row: string[] }> = []
    let units = 0
    let visited = 0
    while (upload.next < upload.rows.length && units < BATCH_UNITS && visited++ < BATCH_ROWS) {
      const index = upload.next++
      const row = upload.rows[index]
      if (upload.previous?.[index] === row) continue
      changes.push({ index, row })
      units += row.length + 1
      for (const cell of row) units += cell.length
    }
    if (!changes.length && upload.next === upload.rows.length) {
      snapshot = upload.rows
      upload = undefined
      sendQuery()
      return
    }
    // 同时在途最多一批；空批次也让出事件循环，不在主线程一次遍历巨型行索引。
    worker!.postMessage({ type: 'rows', version, batch: ++upload.batch, changes })
  }
  function create() {
    const current = new Worker(new URL('./resultFilter.worker.ts', import.meta.url), {
      type: 'module',
    })
    worker = current
    current.onerror = () => {
      if (worker === current) destroy(new Error('数据库筛选工作器运行失败'))
    }
    current.onmessageerror = () => {
      if (worker === current) destroy(new Error('无法读取数据库筛选结果'))
    }
    current.onmessage = (event: MessageEvent<unknown>) => {
      if (worker !== current) return
      try {
        const data = event.data as {
          type: string
          version: number
          id?: number
          batch?: number
          indices?: Uint32Array
        }
        if (
          !data ||
          typeof data !== 'object' ||
          !Number.isSafeInteger(data.version) ||
          !['ready', 'rows', 'result', 'error'].includes(data.type)
        )
          throw new Error('筛选工作器响应格式无效')
        if (data.version !== version) return
        if (data.type === 'error') throw new Error('数据库筛选工作器计算失败')
        if (data.type === 'ready' && upload?.batch === 0) sendBatch()
        else if (data.type === 'rows' && upload && data.batch === upload.batch) sendBatch()
        else if (data.type === 'result' && pending && data.id === pending.id) {
          if (!(data.indices instanceof Uint32Array) || !snapshot)
            throw new Error('筛选工作器响应格式无效')
          let previous = -1
          for (const index of data.indices) {
            if (index <= previous || index >= snapshot.length)
              throw new Error('筛选工作器行索引无效')
            previous = index
          }
          const done = pending
          pending = undefined
          done.resolve(data.indices)
          park()
        }
      } catch (error) {
        destroy(error instanceof Error ? error : new Error('数据库筛选失败'))
      }
    }
  }
  function run(rows: string[][], term: string): Promise<Uint32Array> {
    clearTimeout(idle)
    pending?.reject(abortError())
    pending = undefined
    if (upload && upload.rows !== rows) destroy()
    return new Promise((resolve, reject) => {
      pending = { id: ++sequence, term, resolve, reject }
      try {
        if (!worker) create()
        if (upload) return // 已在传输同一版本，完成后只提交最新关键词。
        if (snapshot === rows) {
          sendQuery()
          return
        }
        upload = {
          rows,
          previous: snapshot?.length === rows.length ? snapshot : undefined,
          next: 0,
          batch: 0,
        }
        worker!.postMessage({
          type: 'begin',
          version: ++version,
          length: rows.length,
          reset: !upload.previous,
        })
      } catch (error) {
        destroy(error instanceof Error ? error : new Error('数据库筛选工作器创建失败'))
      }
    })
  }
  function cancel(keepUpload = false) {
    if (upload && !keepUpload) {
      destroy()
      return
    }
    pending?.reject(abortError())
    pending = undefined
    worker?.postMessage({ type: 'cancel', version })
    if (worker && !upload) park()
  }
  return { run, cancel, destroy }
}
