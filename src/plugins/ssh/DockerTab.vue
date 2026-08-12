<script setup lang="ts">
/**
 * DockerTab · Docker 容器管理子页签（后端 docker 命令真实数据）
 * 搜索（名称/ID/镜像）+ 状态筛选。
 */
import { computed, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, DockerContainer } from './contracts'
import { useUiStore } from '@/stores/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { UiButton, UiSearchInput, UiSelect as Select } from '@/core/ui'
import TerminalTab from './TerminalTab.vue'
import OutputDialog from './OutputDialog.vue'
import DockerTable from './DockerTable.vue'
import { ipc } from './ipc'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
}>()

const ui = useUiStore()

const containers = ref<DockerContainer[]>([])
const keyword = ref('')
const statusFilter = ref<'all' | 'running' | 'exited'>('all')
const terminalContainer = ref<DockerContainer | null>(null)
const logOutput = ref<{ title: string; content: string } | null>(null)
const busyContainerId = ref<string | null>(null)
const pendingAction = ref<{
  container: DockerContainer
  action: 'stop' | 'restart' | 'remove'
} | null>(null)

/** 过滤后的容器列表（搜索 + 状态筛选） */
const filtered = computed(() => {
  let list = containers.value
  const kw = keyword.value.trim().toLowerCase()
  if (kw) {
    list = list.filter(
      (c) =>
        c.name.toLowerCase().includes(kw) ||
        c.id.toLowerCase().includes(kw) ||
        c.image.toLowerCase().includes(kw)
    )
  }
  if (statusFilter.value !== 'all') {
    list = list.filter((c) => c.status === statusFilter.value)
  }
  return list
})

async function refresh() {
  const connectionId = props.connection?.sessionId
  if (!connectionId) return
  try {
    const result = await ipc.sshDockerList(connectionId)
    if (props.connection?.sessionId === connectionId) containers.value = result
  } catch (e) {
    if (props.connection?.sessionId === connectionId) ui.toast(`容器列表加载失败：${e}`)
  }
}

async function action(c: DockerContainer, act: 'start' | 'stop' | 'restart' | 'remove') {
  const connectionId = props.connection?.sessionId
  if (!connectionId || busyContainerId.value) return
  busyContainerId.value = c.id
  try {
    const r = await ipc.sshDockerAction({
      connectionId,
      containerId: c.id,
      action: act,
    })
    if (r.ok) {
      ui.toast(
        `${act === 'start' ? '启动' : act === 'stop' ? '停止' : act === 'restart' ? '重启' : '删除'}容器 ${c.name} 成功`
      )
      await refresh()
    } else {
      ui.toast(`操作失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`操作失败：${e}`)
  } finally {
    busyContainerId.value = null
  }
}

/** 启动无需确认；停止、重启和删除使用项目统一确认弹窗。 */
function requestAction(c: DockerContainer, act: 'start' | 'stop' | 'restart' | 'remove') {
  if (act === 'start') {
    void action(c, act)
    return
  }
  pendingAction.value = { container: c, action: act }
}

function confirmAction() {
  const pending = pendingAction.value
  if (!pending) return
  pendingAction.value = null
  void action(pending.container, pending.action)
}

const pendingActionText = computed(() => {
  const pending = pendingAction.value
  if (!pending) return { title: '', message: '', label: '确定' }
  const name = pending.container.name
  if (pending.action === 'remove') {
    return { title: '删除容器', message: `删除容器「${name}」？此操作不可恢复。`, label: '删除' }
  }
  const verb = pending.action === 'stop' ? '停止' : '重启'
  return { title: `${verb}容器`, message: `确定${verb}容器「${name}」吗？`, label: verb }
})

async function logs(c: DockerContainer) {
  if (!props.connection?.sessionId) return
  try {
    const r = await ipc.sshDockerLogs({
      connectionId: props.connection.sessionId,
      containerId: c.id,
      lines: 100,
    })
    if (r.ok) logOutput.value = { title: `${c.name} · 最近日志`, content: r.logs }
    else ui.toast(`日志获取失败：${r.error ?? '未知错误'}`)
  } catch (e) {
    ui.toast(`日志获取失败：${e}`)
  }
}

function exec(c: DockerContainer) {
  if (!props.connection?.sessionId) return
  terminalContainer.value = c
}

watch(
  () => props.connection?.sessionId,
  (sessionId) => {
    containers.value = []
    if (sessionId) void refresh()
  },
  { immediate: true }
)
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? '未连接' }} · Docker 容器
      </span>
      <UiSearchInput
        v-model="keyword"
        size="sm"
        class="!w-[180px]"
        placeholder="搜索名称/ID/镜像"
      />
      <Select
        :model-value="statusFilter"
        size="sm"
        class="!w-[100px] shrink-0"
        title="按状态筛选"
        :options="[
          { value: 'all', label: '全部' },
          { value: 'running', label: '运行中' },
          { value: 'exited', label: '已停止' },
        ]"
        @update:model-value="statusFilter = $event as 'all' | 'running' | 'exited'"
      />
      <div class="ml-auto">
        <UiButton variant="ghost" size="sm" title="刷新容器列表" @click="refresh"> 刷新 </UiButton>
      </div>
    </div>

    <DockerTable
      :containers="filtered"
      :busy-container-id="busyContainerId"
      @action="requestAction"
      @logs="logs"
      @terminal="exec"
    />

    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>共 {{ filtered.length }} 个容器</span>
      <span class="ml-auto">{{ connection?.status === 'connected' ? '就绪' : '未连接' }}</span>
    </div>

    <Teleport to="body">
      <div
        v-if="terminalContainer"
        class="fixed inset-0 z-[150] grid place-items-center bg-black/30 p-[40px]"
      >
        <div
          class="flex h-full w-full max-w-[900px] flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-[0_16px_48px_rgba(16,24,40,0.25)] dark:border-border-dark dark:bg-surface-dark"
        >
          <div class="flex justify-end border-b border-border p-[6px] dark:border-border-dark">
            <UiButton variant="ghost" size="sm" @click="terminalContainer = null">关闭</UiButton>
          </div>
          <TerminalTab
            :key="terminalContainer.id"
            :connection="connection"
            :profile="profile"
            :docker-container-id="terminalContainer.id"
            :connect-request="1"
          />
        </div>
      </div>
    </Teleport>
    <OutputDialog
      v-if="logOutput"
      :title="logOutput.title"
      :content="logOutput.content"
      @close="logOutput = null"
    />
    <ConfirmDialog
      :open="pendingAction !== null"
      :title="pendingActionText.title"
      :message="pendingActionText.message"
      :confirm-label="pendingActionText.label"
      :danger="pendingAction?.action === 'remove'"
      @close="pendingAction = null"
      @confirm="confirmAction"
    />
  </div>
</template>
