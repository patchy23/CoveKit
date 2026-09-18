<script setup lang="ts">
/** 当前编排的容器快照；请求代次隔离，日志与终端沿用 SSH 现有能力。 */
import { onUnmounted, ref, watch } from 'vue'
import { UiButton, UiModal, UiEmptyState } from '@/core/ui'
import type { DockerContainer, ComposeProject, ServerConnection } from '../contracts'
import { ipc } from '../ipc'
import LiveLogDialog from '../monitor/LiveLogDialog.vue'
import TerminalTab from '../terminal/TerminalTab.vue'
import DockerTable from '../docker/DockerTable.vue'
const props = defineProps<{
  project: ComposeProject
  connection?: ServerConnection
  busy: boolean
}>()
const rows = ref<DockerContainer[]>([])
const error = ref('')
const loading = ref(false)
const logs = ref<DockerContainer>()
const terminal = ref<DockerContainer>()
let version = 0
onUnmounted(() => {
  version++
})
async function refresh() {
  const request = ++version
  if (!props.connection || props.connection.status !== 'connected') {
    rows.value = []
    loading.value = false
    return
  }
  loading.value = true
  error.value = ''
  try {
    const containers = await ipc.sshDockerList(props.connection.sessionId, props.project.name)
    if (request !== version) return
    rows.value = containers
  } catch (e) {
    if (request === version) {
      rows.value = []
      error.value = String(e)
    }
  } finally {
    if (request === version) loading.value = false
  }
}
watch(
  () => [props.project, props.connection?.sessionId, props.connection?.status],
  () => {
    void refresh()
  },
  { immediate: true }
)
watch(
  () => [props.project.name, props.connection?.sessionId, props.connection?.status],
  () => {
    logs.value = terminal.value = undefined
  }
)
defineExpose({ refresh })
</script>

<template>
  <div class="shrink-0">
    <section class="shrink-0 border-b border-border dark:border-border-dark">
      <div
        class="flex items-center justify-between px-md py-xs text-body-sm text-secondary dark:text-secondary-dark"
      >
        <span>{{
          loading
            ? '正在读取容器…'
            : error || connection?.status !== 'connected'
              ? '容器'
              : `容器 · ${rows.length}`
        }}</span>
        <UiButton
          size="xs"
          variant="ghost"
          :loading="loading"
          :disabled="busy || connection?.status !== 'connected'"
          @click="refresh"
          >刷新</UiButton
        >
      </div>
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
        class="px-md pb-sm text-body-sm text-danger-strong dark:text-danger-dark"
      >
        {{ error }}
      </p>
      <div v-else class="flex min-h-0 flex-col">
        <DockerTable
          v-if="rows.length"
          :containers="rows"
          class="max-h-[320px]"
          inspect-only
          :disabled="busy || connection?.status !== 'connected'"
          @logs="logs = $event"
          @terminal="terminal = $event"
        />
        <UiEmptyState
          v-else
          :title="loading ? '正在读取容器…' : '该编排下没有容器'"
          :description="loading ? '' : '未部署或容器已被移除，可启动编排后刷新。'"
        />
      </div>
    </section>
    <LiveLogDialog
      v-if="logs && connection"
      :key="connection.sessionId + logs.id"
      :title="`${logs.name} · 日志`"
      :connection-id="connection.sessionId"
      kind="docker"
      :target-id="logs.id"
      @close="logs = undefined"
    />
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
