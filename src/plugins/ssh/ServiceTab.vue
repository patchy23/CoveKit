<script setup lang="ts">
/**
 * ServiceTab · systemd 服务管理子页签（后端 exec 真实数据）
 */
import { computed, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, SystemdService } from './contracts'
import { useUiStore } from '@/stores/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { UiButton, UiSelect } from '@/core/ui'
import OutputDialog from './OutputDialog.vue'
import { ipc } from './ipc'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
}>()

const ui = useUiStore()

const filter = ref<'all' | 'active' | 'inactive' | 'failed'>('all')
const services = ref<SystemdService[]>([])
const loading = ref(false)
const logOutput = ref<{ title: string; content: string } | null>(null)
const pendingAction = ref<{ service: SystemdService; action: 'stop' | 'restart' } | null>(null)

/** 按状态筛选后的服务列表（computed 自动响应 filter 变化） */
const filtered = computed(() => {
  if (filter.value === 'all') return services.value
  return services.value.filter((s) => s.activeState === filter.value)
})

async function refresh() {
  const connectionId = props.connection?.sessionId
  if (!connectionId) return
  loading.value = true
  try {
    const result = await ipc.sshServiceList({
      connectionId,
      filter: filter.value,
    })
    if (props.connection?.sessionId === connectionId) services.value = result
  } catch (e) {
    if (props.connection?.sessionId === connectionId) ui.toast(`服务列表加载失败：${e}`)
  } finally {
    if (props.connection?.sessionId === connectionId) loading.value = false
  }
}

async function action(svc: SystemdService, act: 'start' | 'stop' | 'restart') {
  if (!props.connection?.sessionId) return
  try {
    const r = await ipc.sshServiceAction({
      connectionId: props.connection.sessionId,
      serviceName: svc.name,
      action: act,
    })
    if (r.ok) {
      ui.toast(`${act === 'start' ? '启动' : act === 'stop' ? '停止' : '重启'} ${svc.name} 成功`)
      refresh()
    } else {
      ui.toast(`${act} ${svc.name} 失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`操作失败：${e}`)
  }
}

function requestAction(svc: SystemdService, act: 'start' | 'stop' | 'restart') {
  if (act === 'start') void action(svc, act)
  else pendingAction.value = { service: svc, action: act }
}

function confirmAction() {
  const pending = pendingAction.value
  pendingAction.value = null
  if (pending) void action(pending.service, pending.action)
}

async function logs(svc: SystemdService) {
  if (!props.connection?.sessionId) return
  try {
    const r = await ipc.sshServiceLogs({
      connectionId: props.connection.sessionId,
      serviceName: svc.name,
      lines: 100,
    })
    if (r.ok) logOutput.value = { title: `${svc.name} · 最近日志`, content: r.logs }
    else ui.toast(`日志获取失败：${r.error ?? '未知错误'}`)
  } catch (e) {
    ui.toast(`日志获取失败：${e}`)
  }
}

function stateClass(s: SystemdService): string {
  if (s.activeState === 'active') return 'text-success-strong dark:text-success-dark'
  if (s.activeState === 'failed') return 'text-danger-strong dark:text-danger-dark'
  return 'text-text-muted dark:text-text-muted-dark'
}

function stateText(s: SystemdService): string {
  if (s.activeState === 'active') return '运行中'
  if (s.activeState === 'failed') return '失败'
  return '已停止'
}

watch(
  () => props.connection?.sessionId,
  (sessionId) => {
    services.value = []
    loading.value = false
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
        {{ profile?.name ?? '未连接' }} · 服务管理
      </span>
      <UiSelect
        :model-value="filter"
        size="sm"
        class="!w-[110px] shrink-0"
        title="按状态筛选"
        :options="[
          { value: 'all', label: '全部' },
          { value: 'active', label: '运行中' },
          { value: 'inactive', label: '已停止' },
          { value: 'failed', label: '失败' },
        ]"
        @update:model-value="filter = $event as 'all' | 'active' | 'inactive' | 'failed'"
      />
      <div class="ml-auto">
        <UiButton variant="ghost" size="sm" title="刷新服务列表" @click="refresh"> 刷新 </UiButton>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto">
      <table class="data-table">
        <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
          <tr
            class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
          >
            <th class="px-[12px] py-[8px] font-medium">服务名</th>
            <th class="px-[12px] py-[8px] font-medium">描述</th>
            <th class="w-[90px] px-[12px] py-[8px] font-medium">状态</th>
            <th class="w-[140px] px-[12px] py-[8px] font-medium">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="s in filtered"
            :key="s.name"
            class="border-b border-border/50 transition-colors hover:bg-surface-muted dark:border-border-dark/50 dark:hover:bg-surface-muted-dark"
          >
            <td class="data-cell-tech px-[12px] py-[8px]">{{ s.name }}</td>
            <td class="data-cell-tech px-[12px] py-[8px]">{{ s.description }}</td>
            <td class="data-cell-text px-[12px] py-[8px]">
              <span :class="stateClass(s)">{{ stateText(s) }}</span>
            </td>
            <td class="data-cell-action px-[12px] py-[8px]">
              <div class="flex gap-[4px]">
                <UiButton
                  v-if="s.activeState !== 'active'"
                  variant="ghost"
                  size="xs"
                  @click="requestAction(s, 'start')"
                >
                  启动
                </UiButton>
                <UiButton
                  v-if="s.activeState === 'active'"
                  variant="ghost"
                  size="xs"
                  @click="requestAction(s, 'stop')"
                >
                  停止
                </UiButton>
                <UiButton
                  v-if="s.activeState === 'active'"
                  variant="ghost"
                  size="xs"
                  @click="requestAction(s, 'restart')"
                >
                  重启
                </UiButton>
                <UiButton variant="ghost" size="xs" @click="logs(s)"> 日志 </UiButton>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>共 {{ filtered.length }} 个服务</span>
      <span class="ml-auto">{{ connection?.status === 'connected' ? '就绪' : '未连接' }}</span>
    </div>
    <OutputDialog
      v-if="logOutput"
      :title="logOutput.title"
      :content="logOutput.content"
      @close="logOutput = null"
    />
    <ConfirmDialog
      :open="pendingAction !== null"
      :title="`${pendingAction?.action === 'stop' ? '停止' : '重启'}服务`"
      :message="`确定${pendingAction?.action === 'stop' ? '停止' : '重启'}服务「${pendingAction?.service.name ?? ''}」吗？`"
      :confirm-label="pendingAction?.action === 'stop' ? '停止' : '重启'"
      @close="pendingAction = null"
      @confirm="confirmAction"
    />
  </div>
</template>
