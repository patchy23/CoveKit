/** 容器列表请求归当前连接所有；同连接重复刷新合并为下一次即时读取。 */
import { onBeforeUnmount, ref, watch } from 'vue'
import type { DockerContainer } from '../contracts'
import { ipc } from '../ipc'

export function useDockerContainers(
  getConnectionId: () => string | undefined,
  report: (message: string) => void
) {
  const containers = ref<DockerContainer[]>([])
  let disposed = false
  let generation = 0
  let running: { pending: boolean; promise: Promise<void> } | undefined

  function refresh(): Promise<void> {
    const connectionId = getConnectionId()
    if (disposed || !connectionId) return Promise.resolve()
    if (running) {
      // 后来的刷新可能发生在容器操作之后，不能仅返回操作前的旧快照。
      running.pending = true
      return running.promise
    }
    const owner = generation
    const job = { pending: false, promise: Promise.resolve() }
    running = job
    const isCurrent = () => !disposed && generation === owner && getConnectionId() === connectionId
    job.promise = (async () => {
      try {
        do {
          job.pending = false
          try {
            const result = await ipc.sshDockerList(connectionId)
            if (isCurrent() && !job.pending) containers.value = result
          } catch (error) {
            if (isCurrent() && !job.pending) report(`容器列表加载失败：${error}`)
          }
        } while (isCurrent() && job.pending)
      } finally {
        if (running === job) running = undefined
      }
    })()
    return job.promise
  }

  watch(
    getConnectionId,
    () => {
      generation++
      running = undefined
      containers.value = []
      void refresh()
    },
    { immediate: true, flush: 'sync' }
  )
  onBeforeUnmount(() => {
    disposed = true
    generation++
    running = undefined
  })
  return { containers, refresh }
}
