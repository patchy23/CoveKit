import type { LogSnapshot } from '../contracts'
import { ipc } from '../ipc'

/** 仅合并相同在途读取；完成或失败即释放，不缓存正文、不延长轮询周期。 */
const pending = new Map<string, Promise<LogSnapshot>>()

export function readLogSnapshot(
  kind: 'service' | 'docker',
  connectionId: string,
  targetId: string,
  lines: number,
  previousFingerprint?: string
): Promise<LogSnapshot> {
  // 基线也必须相同，否则无变化响应可能被交给没有对应正文的窗口。
  const key = JSON.stringify([kind, connectionId, targetId, lines, previousFingerprint || null])
  const existing = pending.get(key)
  if (existing) return existing
  const common = {
    connectionId,
    lines,
    ...(previousFingerprint ? { previousFingerprint } : {}),
  }
  const request = (
    kind === 'service'
      ? ipc.sshServiceLogs({ ...common, serviceName: targetId })
      : ipc.sshDockerLogs({ ...common, containerId: targetId })
  ).finally(() => pending.delete(key))
  pending.set(key, request)
  return request
}
