import { inject, onUnmounted, provide, ref, watch, type InjectionKey } from 'vue'

export interface SshLogTarget {
  connectionId: string
  kind: 'service' | 'docker'
  targetId: string
  title: string
}
type LogWindow = SshLogTarget & { key: string; activation: number }
const key: InjectionKey<(target: SshLogTarget) => void> = Symbol('ssh-log-windows')

/** SSH 工作区拥有窗口，子页签卸载不释放日志；会话失效则销毁窗口及轮询。 */
export function provideLogWindows(sessions: () => { sessionId: string; title: string }[]) {
  const windows = ref<LogWindow[]>([])
  function open(target: SshLogTarget) {
    const session = sessions().find((item) => item.sessionId === target.connectionId)
    if (!session) return
    const id = JSON.stringify([target.connectionId, target.kind, target.targetId])
    const existing = windows.value.find((item) => item.key === id)
    if (existing) {
      existing.activation++
      return
    }
    windows.value.push({
      ...target,
      title: `${session.title} · ${target.title}`,
      key: id,
      activation: 0,
    })
  }
  function close(id: string) {
    windows.value = windows.value.filter((item) => item.key !== id)
  }
  watch(
    () => sessions().map((item) => item.sessionId),
    (ids) => {
      windows.value = windows.value.filter((item) => ids.includes(item.connectionId))
    }
  )
  provide(key, open)
  onUnmounted(() => {
    windows.value = []
  })
  return { windows, open, close }
}

export function useLogWindows() {
  // 独立挂载的预览或测试允许无宿主；生产入口始终由 SSH 根提供。
  const open = inject(key, undefined)
  return (target: SshLogTarget) => {
    if (!open) throw new Error('日志窗口需要 SSH 工作区宿主')
    open(target)
  }
}
