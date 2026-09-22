/** 资源计数聚合：CPU 以整机逻辑核心总能力为 100%，PID 复用不继承旧时间。 */
import type { ResourceSnapshot } from '@/core/ipc/contracts'

export interface TimedSnapshot {
  time: number
  value: ResourceSnapshot
}

export function resourceTotals(current: TimedSnapshot, previous?: TimedSnapshot) {
  const processes = current.value.processes
  const memoryKey =
    current.value.memoryMetric === 'privateWorkingSet' ? 'privateResidentBytes' : 'residentBytes'
  const memoryTotal = (group = processes) =>
    group.every((p) => p[memoryKey] !== null)
      ? group.reduce((sum, p) => sum + (p[memoryKey] ?? 0), 0)
      : null
  const sumOptional = (key: 'privateResidentBytes' | 'privateBytes' | 'threads' | 'handles') =>
    processes.every((p) => p[key] !== null)
      ? processes.reduce((sum, p) => sum + (p[key] ?? 0), 0)
      : null
  let cpu: number | null = null
  if (previous && current.time > previous.time && current.value.logicalCpus > 0) {
    const old = new Map(
      previous.value.processes.map((p) => [`${p.pid}:${p.identity}`, p.cpuSeconds])
    )
    let matched = 0
    let seconds = 0
    for (const process of processes) {
      const start = old.get(`${process.pid}:${process.identity}`)
      if (start === undefined || process.cpuSeconds < start) continue
      matched++
      seconds += process.cpuSeconds - start
    }
    if (matched)
      cpu = Math.min(
        100,
        (seconds / ((current.time - previous.time) / 1000) / current.value.logicalCpus) * 100
      )
  }
  return {
    memory: memoryTotal(),
    resident: processes.reduce((sum, p) => sum + p.residentBytes, 0),
    main: memoryTotal(processes.filter((p) => p.kind === 'main')),
    webview: memoryTotal(processes.filter((p) => p.kind === 'webview')),
    child: memoryTotal(processes.filter((p) => p.kind === 'child')),
    privateResident: sumOptional('privateResidentBytes'),
    privateBytes: sumOptional('privateBytes'),
    threads: sumOptional('threads'),
    handles: sumOptional('handles'),
    cpu,
    processes: processes.length,
  }
}

export function memoryLabel(bytes: number | null | undefined) {
  if (bytes == null) return '—'
  return bytes >= 1024 ** 3
    ? `${(bytes / 1024 ** 3).toFixed(2)} GiB`
    : `${Math.round(bytes / 1024 ** 2)} MiB`
}

/** 单一串行循环；停止后的迟到回包无效，重新开启不会与旧请求重叠。 */
export function createResourcePoller<T>(
  read: () => Promise<T>,
  receive: (value: T) => void,
  fail: (error: unknown) => void
) {
  let running = false
  let busy = false
  let generation = 0
  let timer: ReturnType<typeof setTimeout> | undefined
  async function tick() {
    if (!running || busy) return
    busy = true
    const current = generation
    try {
      const value = await read()
      if (running && current === generation) receive(value)
    } catch (error) {
      if (running && current === generation) fail(error)
    } finally {
      busy = false
      if (running) timer = setTimeout(() => void tick(), current === generation ? 1000 : 0)
    }
  }
  return {
    start() {
      if (running) return
      running = true
      generation++
      void tick()
    },
    stop() {
      running = false
      generation++
      clearTimeout(timer)
    },
  }
}
