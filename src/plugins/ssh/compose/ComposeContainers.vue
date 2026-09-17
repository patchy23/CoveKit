<script setup lang="ts">
/** 当前编排的容器快照；请求代次隔离，日志与终端沿用 SSH 现有能力。 */
import { onUnmounted, ref, watch } from 'vue'
import { UiButton, UiModal, UiScrollArea, UiTable, UiTableCell } from '@/core/ui'
import type { ComposeContainer, ComposeProject, ServerConnection } from '../contracts'
import { ipc } from '../ipc'
import { parseComposeContainers } from './composeContainers'
import LiveLogDialog from '../monitor/LiveLogDialog.vue'
import TerminalTab from '../terminal/TerminalTab.vue'
const props = defineProps<{
  project: ComposeProject
  connection?: ServerConnection
  busy: boolean
}>()
const rows = ref<ComposeContainer[]>([])
const error = ref('')
const loading = ref(false)
const logs = ref<ComposeContainer>()
const terminal = ref<ComposeContainer>()
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
    const result = await ipc.sshComposeAction({
      connectionId: props.connection.sessionId,
      project: props.project,
      action: 'ps',
    })
    if (request !== version) return
    if (result.exitCode !== 0)
      throw new Error(result.stderr || result.stdout || `退出码 ${result.exitCode}`)
    rows.value = parseComposeContainers(result.stdout)
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
        <span>容器 · {{ rows.length }}</span>
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
        v-if="error"
        role="alert"
        class="px-md pb-sm text-body-sm text-danger-strong dark:text-danger-dark"
      >
        {{ error }}
      </p>
      <UiScrollArea v-else class="max-h-[190px]" axis="both">
        <UiTable v-if="rows.length" :framed="false" density="compact">
          <thead>
            <tr>
              <UiTableCell
                v-for="title in ['服务 / 容器', '状态', '镜像', '端口', '操作']"
                :key="title"
                as="th"
                >{{ title }}</UiTableCell
              >
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in rows" :key="row.id">
              <UiTableCell content="technical"
                ><div>{{ row.service }}</div>
                <div class="text-text-muted dark:text-text-muted-dark">
                  {{ row.name }}
                </div></UiTableCell
              >
              <UiTableCell
                >{{ row.state }}<span v-if="row.health"> · {{ row.health }}</span></UiTableCell
              >
              <UiTableCell content="technical">{{ row.image }}</UiTableCell
              ><UiTableCell content="technical">{{ row.ports || '—' }}</UiTableCell>
              <UiTableCell content="action"
                ><div class="flex gap-xs">
                  <UiButton size="xs" variant="ghost" @click="logs = row">日志</UiButton
                  ><UiButton
                    size="xs"
                    variant="ghost"
                    :disabled="row.state !== 'running'"
                    @click="terminal = row"
                    >终端</UiButton
                  >
                </div></UiTableCell
              >
            </tr>
          </tbody>
        </UiTable>
        <p v-else class="px-md py-sm text-body-sm text-text-muted dark:text-text-muted-dark">
          {{ loading ? '正在读取容器…' : '尚未创建容器，启动后显示运行状态。' }}
        </p>
      </UiScrollArea>
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
