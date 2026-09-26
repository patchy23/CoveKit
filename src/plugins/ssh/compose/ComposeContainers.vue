<script setup lang="ts">
/** 当前编排的容器快照；请求代次隔离，日志与终端沿用 SSH 现有能力。 */
import { onBeforeUnmount, ref, watch } from 'vue'
import { UiModal, UiEmptyState } from '@/core/ui'
import type { DockerContainer, ComposeProject, ServerConnection } from '../contracts'
import { ipc } from '../ipc'
import { useLogWindows } from '../monitor/logWindows'
import TerminalTab from '../terminal/TerminalTab.vue'
import DockerTable from '../docker/DockerTable.vue'
import { parseComposeContainers } from './composeContainers'
const props = defineProps<{
  project: ComposeProject
  connection?: ServerConnection
  busy: boolean
}>()
const rows = ref<DockerContainer[]>([])
const error = ref('')
const loading = ref(false)
const openLog = useLogWindows()
function logs(container: DockerContainer) {
  if (!props.connection) return
  openLog({
    connectionId: props.connection.sessionId,
    kind: 'docker',
    targetId: container.id,
    title: `${container.name} · 日志`,
  })
}
const terminal = ref<DockerContainer>()
let version = 0
let disposed = false
let sourceKey = ''
let job: { pending: boolean; promise: Promise<void> } | undefined
onBeforeUnmount(() => {
  disposed = true
  version++
  job = undefined
})
function refresh(): Promise<void> {
  if (disposed) return Promise.resolve()
  if (!props.connection || props.connection.status !== 'connected') {
    rows.value = []
    loading.value = false
    return Promise.resolve()
  }
  if (job) {
    job.pending = true
    return job.promise
  }
  const request = version
  const connectionId = props.connection.sessionId
  const current = { pending: false, promise: Promise.resolve() }
  job = current
  const isCurrent = () => !disposed && request === version
  loading.value = true
  error.value = ''
  current.promise = (async () => {
    try {
      do {
        current.pending = false
        try {
          const result = await ipc.sshComposeAction({
            connectionId,
            project: props.project,
            action: 'ps',
          })
          if (isCurrent()) {
            rows.value = parseComposeContainers(result)
            error.value = ''
          }
        } catch (e) {
          if (isCurrent()) {
            rows.value = []
            error.value = String(e)
          }
        }
      } while (isCurrent() && current.pending)
    } finally {
      if (job === current) job = undefined
      if (isCurrent()) loading.value = false
    }
  })()
  return current.promise
}
watch(
  () => [props.project, props.connection?.sessionId, props.connection?.status],
  () => {
    const nextKey = JSON.stringify([
      props.connection?.sessionId,
      props.connection?.status,
      props.project.name,
      props.project.configFiles,
      props.project.workingDir,
    ])
    if (nextKey !== sourceKey) {
      sourceKey = nextKey
      version++
      job = undefined
      rows.value = []
      error.value = ''
    }
    void refresh()
  },
  { immediate: true }
)
watch(
  () => [props.project.name, props.connection?.sessionId, props.connection?.status],
  () => {
    terminal.value = undefined
  }
)
</script>

<template>
  <div class="shrink-0">
    <section class="shrink-0" :aria-busy="loading">
      <p
        v-if="connection?.status !== 'connected'"
        role="status"
        class="px-md py-sm text-body-sm text-warning-strong dark:text-warning-dark"
      >
        SSH 已断开，恢复连接后重新读取容器。
      </p>
      <p
        v-else-if="error"
        role="alert"
        class="select-text px-md py-sm text-body-sm text-danger-strong dark:text-danger-dark"
      >
        {{ error }}
      </p>
      <div v-else class="flex min-h-0 flex-col">
        <DockerTable
          v-if="rows.length || loading"
          :containers="rows"
          class="max-h-[320px]"
          inspect-only
          compact
          :disabled="busy || connection?.status !== 'connected'"
          @logs="logs"
          @terminal="terminal = $event"
        />
        <UiEmptyState
          v-else
          compact
          title="该编排下没有容器"
          description="未部署或容器已被移除，可启动编排后刷新。"
        />
      </div>
    </section>
    <UiModal
      v-if="terminal"
      open
      size="xl"
      :title="`${terminal.name} · 终端`"
      @close="terminal = undefined"
    >
      <div class="h-[60vh]">
        <TerminalTab
          :key="terminal.id"
          :connection="connection"
          :docker-container-id="terminal.id"
          :connect-request="1"
        />
      </div>
    </UiModal>
  </div>
</template>
