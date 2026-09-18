<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * ServiceTab · systemd 服务管理子页签（后端 exec 真实数据）
 */
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import type { ServerConnection, ServerProfile, SystemdService } from '../contracts'
import { useUiStore } from '@/stores/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import {
  UiButton,
  UiSearchInput,
  UiSelect,
  UiStatusBar,
  UiTable,
  UiTableCell,
  UiToolbar,
} from '@/core/ui'
import LiveLogDialog from './LiveLogDialog.vue'
import ServiceConfigDialog from './ServiceConfigDialog.vue'
import { ipc } from '../ipc'
import RuntimeStatus from '../RuntimeStatus.vue'

const props = defineProps<{
  connection?: ServerConnection
  profile?: ServerProfile
}>()

const ui = useUiStore()

const filter = ref<'all' | 'active' | 'inactive' | 'failed'>('all')
const services = ref<SystemdService[]>([])
const query = ref('')
const configTarget = ref<SystemdService | null>(null)
let refreshSequence = 0
const loading = ref(false)
const logTarget = ref<SystemdService | null>(null)
const pendingAction = ref<{ service: SystemdService; action: 'stop' | 'restart' } | null>(null)

/** 按状态筛选后的服务列表（computed 自动响应 filter 变化） */
const filtered = computed(() => {
  const keyword = query.value.trim().toLocaleLowerCase()
  return services.value.filter(
    (service) =>
      (filter.value === 'all' || service.activeState === filter.value) &&
      (!keyword || `${service.name} ${service.description}`.toLocaleLowerCase().includes(keyword))
  )
})

async function refresh() {
  const connectionId = props.connection?.sessionId
  if (!connectionId) return
  const sequence = ++refreshSequence
  loading.value = true
  try {
    const result = await ipc.sshServiceList({
      connectionId,
      filter: 'all',
    })
    if (sequence === refreshSequence && props.connection?.sessionId === connectionId)
      services.value = result
  } catch (e) {
    if (sequence === refreshSequence && props.connection?.sessionId === connectionId)
      ui.toast(`服务列表加载失败：${e}`)
  } finally {
    if (sequence === refreshSequence && props.connection?.sessionId === connectionId)
      loading.value = false
  }
}

async function action(svc: SystemdService, act: 'start' | 'stop' | 'restart') {
  const connectionId = props.connection?.sessionId
  if (!connectionId) return
  try {
    const r = await ipc.sshServiceAction({
      connectionId,
      serviceName: svc.name,
      action: act,
    })
    if (props.connection?.sessionId !== connectionId) return
    if (r.ok) {
      ui.toast(`${act === 'start' ? '启动' : act === 'stop' ? '停止' : '重启'} ${svc.name} 成功`)
      refresh()
    } else {
      ui.toast(`${act} ${svc.name} 失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    if (props.connection?.sessionId === connectionId) ui.toast(`操作失败：${e}`)
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

function logs(service: SystemdService) {
  logTarget.value = service
}

watch(
  () => props.connection?.sessionId,
  (sessionId) => {
    refreshSequence += 1
    configTarget.value = null
    logTarget.value = null
    pendingAction.value = null
    services.value = []
    loading.value = false
    if (sessionId) void refresh()
  },
  { immediate: true }
)
onBeforeUnmount(() => {
  refreshSequence += 1
})
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <UiToolbar bordered title="服务管理">
      <UiSearchInput
        v-model="query"
        size="sm"
        class="!w-[180px]"
        placeholder="搜索服务名称或描述"
      />
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
      <template #trailing>
        <UiButton
          variant="ghost"
          size="sm"
          title="刷新服务列表"
          :loading="loading"
          @click="refresh"
        >
          刷新
        </UiButton>
      </template>
    </UiToolbar>

    <UiScrollArea as-child axis="vertical">
      <div class="min-h-0 flex-1">
        <UiTable :framed="false" :styled="false" table-class="text-body-sm">
          <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
            <tr
              class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
            >
              <UiTableCell as="th" class="px-[12px] py-[8px]">服务名</UiTableCell>
              <UiTableCell as="th" class="px-[12px] py-[8px]">描述</UiTableCell>
              <UiTableCell as="th" class="w-[90px] px-[12px] py-[8px]">状态</UiTableCell>
              <UiTableCell as="th" class="w-[190px] px-[12px] py-[8px]">操作</UiTableCell>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="s in filtered"
              :key="s.name"
              class="border-b border-border/50 transition-colors dark:border-border-dark/50"
            >
              <UiTableCell content="technical" class="px-[12px] py-[8px]">{{ s.name }}</UiTableCell>
              <UiTableCell content="technical" class="px-[12px] py-[8px]">{{
                s.description
              }}</UiTableCell>
              <UiTableCell content="status" class="px-[12px] py-[8px]">
                <RuntimeStatus :status="s.activeState" />
              </UiTableCell>
              <UiTableCell content="action" class="px-[12px] py-[8px]">
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
                  <UiButton variant="ghost" size="xs" @click="configTarget = s">配置</UiButton>
                </div>
              </UiTableCell>
            </tr>
          </tbody>
        </UiTable>
      </div>
    </UiScrollArea>

    <UiStatusBar>
      <span>共 {{ filtered.length }} 个服务</span>
      <template #trailing>
        <span>{{ connection?.status === 'connected' ? '就绪' : '未连接' }}</span>
      </template>
    </UiStatusBar>
    <LiveLogDialog
      v-if="logTarget && connection"
      :title="`${logTarget.name} · 实时日志`"
      :connection-id="connection.sessionId"
      kind="service"
      :target-id="logTarget.name"
      @close="logTarget = null"
    />
    <ServiceConfigDialog
      v-if="configTarget && connection"
      :connection-id="connection.sessionId"
      :service-name="configTarget.name"
      @close="configTarget = null"
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
