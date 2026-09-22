/** 诊断只保存数字与归属，不持有业务对象、IPC 正文或组件引用。 */
export interface ToolMetrics {
  requests: number
  failures: number
  inFlight: number
  totalMs: number
  completed: number
}
export interface ResourceCounts {
  scopes: number
  listeners: number
  timers: number
}
const commands = new Map<string, string>()
const measured = new Map<string, ToolMetrics>()
const resources = new Map<string, ResourceCounts>()

/** 元数据来自后端现有模块清单，禁止在前端复制命令归属表。 */
export function setMonitorCommands(entries: { name: string; toolId: string }[]) {
  commands.clear()
  for (const entry of entries) commands.set(entry.name, entry.toolId)
}

/** 取消勾选即丢弃该工具的会话统计；迟到请求只能更新已脱离注册表的旧计数。 */
export function configureToolMetrics(ids: string[]) {
  for (const id of measured.keys()) if (!ids.includes(id)) measured.delete(id)
  for (const id of ids) {
    if (!measured.has(id))
      measured.set(id, { requests: 0, failures: 0, inFlight: 0, totalMs: 0, completed: 0 })
  }
}

/** 只包装已启用工具的 IPC；时间是端到端响应时间，不是 CPU 时间。 */
export function beginMeasuredCommand(command: string): ((failed: boolean) => void) | undefined {
  const owner = commands.get(command)
  const stats = owner ? measured.get(owner) : undefined
  if (!stats) return
  const start = performance.now()
  stats.requests++
  stats.inFlight++
  let finished = false
  return (failed) => {
    if (finished) return
    finished = true
    stats.inFlight--
    stats.completed++
    stats.totalMs += Math.max(0, performance.now() - start)
    if (failed) stats.failures++
  }
}

/** 跟随既有 scope 创建/释放维护少量数字，监测晚开启也能看到现存资源。 */
export function countScopeResource(owner: string, key: keyof ResourceCounts, delta: number) {
  const value = resources.get(owner) ?? { scopes: 0, listeners: 0, timers: 0 }
  value[key] += delta
  if (!value.scopes && !value.listeners && !value.timers) resources.delete(owner)
  else resources.set(owner, value)
}

/** 返回数字快照；仅统计接入 scope 的订阅/定时器，不宣称全工具覆盖。 */
export function toolMetricSnapshot(id: string): ToolMetrics & ResourceCounts {
  return {
    requests: 0,
    failures: 0,
    inFlight: 0,
    totalMs: 0,
    completed: 0,
    scopes: 0,
    listeners: 0,
    timers: 0,
    ...measured.get(id),
    ...resources.get(id),
  }
}
