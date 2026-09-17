<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { UiTooltip } from '@/core/ui'
/**
 * TunnelTab · SSH 端口隧道（-L 本地 / -R 远程 / -D 动态 SOCKS5）
 * 列表 = 该服务器的全部隧道配置 + 运行时状态（事件推送更新）；
 * 新建/编辑 = 弹窗表单；启停/删除 = 行内操作。
 * 安全约束：监听 0.0.0.0/:: 时橙色警告 + 需输入主机名解锁保存。
 */
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import type {
  ServerConnection,
  ServerProfile,
  TunnelConfig,
  TunnelRuntime,
  TunnelType,
} from '../contracts'
import { ipc, onTunnelStatus } from '../ipc'
import {
  UiButton,
  UiCheckbox,
  UiField,
  UiIcon,
  UiIconButton,
  UiInput,
  UiModal,
  UiSelect as Select,
} from '@/core/ui'
import { useUiStore } from '@/stores/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'

const props = defineProps<{
  connection: ServerConnection
  profile?: ServerProfile
}>()

const ui = useUiStore()

const configs = ref<TunnelConfig[]>([])
const runtimes = ref<Map<string, TunnelRuntime>>(new Map())
const loaded = ref(false)
const loadFailed = ref(false)

const formOpen = ref(false)
const editingId = ref<string | null>(null)
const deleteTarget = ref<TunnelConfig | null>(null)
/** 保存解锁输入（公网监听时需输入主机名） */
const dangerConfirmText = ref('')
/** 频繁操作防抖：进行中的 tunnelId 集合 */
const busyIds = reactive(new Set<string>())

const form = reactive({
  name: '',
  tunnelType: 'local' as TunnelType,
  listenHost: '127.0.0.1',
  listenPort: 8080,
  targetHost: '127.0.0.1',
  targetPort: 80,
  autoStart: false,
})

const TYPE_OPTIONS = [
  { value: 'local', label: '本地转发 -L（本地端口 → 远程目标）' },
  { value: 'remote', label: '远程转发 -R（服务端端口 → 本机目标）' },
  { value: 'dynamic', label: '动态 SOCKS5 -D（本地代理）' },
]

const isDangerListen = computed(() => ['0.0.0.0', '::'].includes(form.listenHost.trim()))
const canSave = computed(
  () => !isDangerListen.value || dangerConfirmText.value.trim() === (props.profile?.host ?? '___')
)

const TYPE_SHORT: Record<TunnelType, string> = { local: 'L', remote: 'R', dynamic: 'D' }
const TYPE_LABEL: Record<TunnelType, string> = { local: '本地', remote: '远程', dynamic: 'SOCKS5' }

/** 配置 + 运行态合并视图（按配置顺序；未运行的显示 stopped） */
const rows = computed(() => {
  return configs.value.map((config) => ({
    config,
    runtime: runtimes.value.get(config.id),
  }))
})

function runtimeOf(config: TunnelConfig): TunnelRuntime | undefined {
  return runtimes.value.get(config.id)
}

const STATUS_LABEL: Record<string, string> = {
  stopped: '已停止',
  starting: '启动中',
  running: '运行中',
  error: '异常',
}
const STATUS_CLASS: Record<string, string> = {
  stopped: 'bg-neutral text-text-muted dark:bg-neutral-dark dark:text-text-muted-dark',
  starting:
    'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark',
  running: 'bg-success-soft text-success-strong dark:bg-success-soft-dark dark:text-success-dark',
  error: 'bg-danger-soft text-danger-strong dark:bg-danger-soft-dark dark:text-danger-dark',
}

/** 监听展示：本地/远程显示 host:port，SOCKS5 显示 proxy 标记 */
function listenText(config: TunnelConfig): string {
  return `${config.listenHost}:${config.listenPort}`
}

function targetText(config: TunnelConfig): string {
  if (config.tunnelType === 'dynamic') return '按客户端请求'
  return `${config.targetHost ?? ''}:${config.targetPort ?? ''}`
}

async function load() {
  try {
    configs.value = await ipc.sshTunnelList(props.profile?.id ?? '')
    // 运行态快照兜底（事件负责增量更新）
    if (props.connection.sessionId) {
      const list = await ipc.sshTunnels(props.connection.sessionId)
      const next = new Map(runtimes.value)
      for (const r of list) next.set(r.id, r)
      runtimes.value = next
    }
    loadFailed.value = false
  } catch {
    loadFailed.value = true
  } finally {
    loaded.value = true
  }
}

function openCreate() {
  editingId.value = null
  Object.assign(form, {
    name: '',
    tunnelType: 'local',
    listenHost: '127.0.0.1',
    listenPort: 8080,
    targetHost: '127.0.0.1',
    targetPort: 80,
    autoStart: false,
  })
  dangerConfirmText.value = ''
  formOpen.value = true
}

function openEdit(config: TunnelConfig) {
  editingId.value = config.id
  Object.assign(form, {
    name: config.name,
    tunnelType: config.tunnelType,
    listenHost: config.listenHost,
    listenPort: config.listenPort,
    targetHost: config.targetHost ?? '127.0.0.1',
    targetPort: config.targetPort ?? 80,
    autoStart: config.autoStart,
  })
  dangerConfirmText.value = ''
  formOpen.value = true
}

async function save() {
  if (!form.name.trim()) {
    ui.toast('请输入隧道名称')
    return
  }
  if (!Number.isInteger(form.listenPort) || form.listenPort < 1 || form.listenPort > 65535) {
    ui.toast('监听端口必须是 1 到 65535 之间的整数')
    return
  }
  if (form.tunnelType !== 'dynamic') {
    if (!form.targetHost.trim()) {
      ui.toast('请输入目标地址')
      return
    }
    if (!Number.isInteger(form.targetPort) || form.targetPort < 1 || form.targetPort > 65535) {
      ui.toast('目标端口必须是 1 到 65535 之间的整数')
      return
    }
  }
  if (isDangerListen.value && !canSave.value) {
    ui.toast(`监听公网地址需输入主机名「${props.profile?.host ?? ''}」确认`)
    return
  }
  const config: TunnelConfig = {
    id: editingId.value ?? `tun-${Date.now()}`,
    profileId: props.profile?.id ?? '',
    name: form.name.trim(),
    tunnelType: form.tunnelType,
    listenHost: form.listenHost.trim() || '127.0.0.1',
    listenPort: form.listenPort,
    targetHost: form.tunnelType === 'dynamic' ? undefined : form.targetHost.trim(),
    targetPort: form.tunnelType === 'dynamic' ? undefined : form.targetPort,
    autoStart: form.autoStart,
  }
  try {
    const saved = await ipc.sshTunnelSave(config)
    const index = configs.value.findIndex((c) => c.id === saved.id)
    if (index >= 0) configs.value[index] = saved
    else configs.value.push(saved)
    formOpen.value = false
    ui.toast(`${index >= 0 ? '已更新' : '已添加'}隧道「${saved.name}」`)
  } catch (error) {
    ui.toast(`保存失败：${error}`)
  }
}

async function toggleStart(config: TunnelConfig) {
  if (busyIds.has(config.id)) return
  const runtime = runtimeOf(config)
  const running = runtime?.status === 'running' || runtime?.status === 'starting'
  busyIds.add(config.id)
  try {
    const result = running
      ? await ipc.sshTunnelStop(config.id)
      : await ipc.sshTunnelStart(props.connection.sessionId, config.id)
    const next = new Map(runtimes.value)
    next.set(config.id, result)
    runtimes.value = next
    if (result.status === 'error') ui.toast(`隧道「${config.name}」${result.error ?? '启动失败'}`)
  } catch (error) {
    ui.toast(`${running ? '停止' : '启动'}失败：${error}`)
  } finally {
    busyIds.delete(config.id)
  }
}

async function removeTunnel(config: TunnelConfig) {
  try {
    await ipc.sshTunnelDelete(config.id)
    configs.value = configs.value.filter((c) => c.id !== config.id)
    const next = new Map(runtimes.value)
    next.delete(config.id)
    runtimes.value = next
    ui.toast(`已删除隧道「${config.name}」`)
  } catch (error) {
    ui.toast(`删除失败：${error}`)
  }
}

let unlisten: (() => void) | null = null
let disposed = false

onMounted(async () => {
  await load()
  try {
    const stop = await onTunnelStatus((runtime) => {
      if (runtime.profileId !== props.profile?.id) return
      const next = new Map(runtimes.value)
      next.set(runtime.id, runtime)
      runtimes.value = next
    })
    if (disposed) stop()
    else unlisten = stop
  } catch {
    /* 浏览器预览没有 Tauri 事件系统。 */
  }
})

onUnmounted(() => {
  disposed = true
  unlisten?.()
})
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 工具栏 -->
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">隧道</span>
      <span class="font-mono text-caption text-text-muted dark:text-text-muted-dark">
        {{ connection.host ?? '' }}
      </span>
      <div class="ml-auto">
        <UiButton
          variant="ghost"
          size="xs"
          class="!h-auto !px-[8px] !py-[3px] text-caption"
          @click="openCreate"
        >
          <UiIcon name="plus" :size="12" class="mr-[3px]" />新建隧道
        </UiButton>
      </div>
    </div>

    <!-- 列表 -->
    <UiScrollArea as-child axis="vertical">
      <div class="min-h-0 flex-1 p-[12px]">
        <div
          v-if="loadFailed"
          class="py-[24px] text-center text-body-sm text-danger-strong dark:text-danger-dark"
        >
          隧道配置加载失败，请重试。
        </div>
        <div
          v-else-if="loaded && rows.length === 0"
          class="flex flex-col items-center justify-center py-[64px] text-center"
        >
          <div
            class="grid h-11 w-11 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
          >
            <UiIcon
              name="play-all"
              :size="20"
              class="text-tertiary-strong dark:text-tertiary-dark"
            />
          </div>
          <p class="mt-md text-body font-medium dark:text-primary-dark">还没有隧道</p>
          <p class="mt-[4px] max-w-[360px] text-body-sm text-text-muted dark:text-text-muted-dark">
            端口转发可以把远程服务映射到本机（-L）、把本机服务暴露给服务器（-R），或建立 SOCKS5
            代理（-D）。
          </p>
          <UiButton variant="ghost" size="sm" class="mt-[12px]" @click="openCreate"
            >新建第一条隧道</UiButton
          >
        </div>

        <div v-else class="overflow-hidden rounded-lg border border-border dark:border-border-dark">
          <div
            v-for="{ config } in rows"
            :key="config.id"
            class="flex items-center gap-[12px] border-b border-border bg-surface px-[14px] py-[10px] last:border-b-0 dark:border-border-dark dark:bg-surface-dark"
          >
            <!-- 类型徽标 -->
            <UiTooltip :content="TYPE_LABEL[config.tunnelType]">
              <span
                class="grid h-[30px] w-[30px] shrink-0 place-items-center rounded-[8px] bg-tertiary-soft font-mono text-body-sm font-bold text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
              >
                {{ TYPE_SHORT[config.tunnelType] }}
              </span>
            </UiTooltip>
            <!-- 名称 + 地址 -->
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-[8px]">
                <span class="truncate text-body font-medium text-primary dark:text-primary-dark">
                  {{ config.name }}
                </span>
                <UiTooltip v-if="config.autoStart" content="连接建立后自动启动">
                  <span
                    class="shrink-0 rounded-full bg-neutral px-[7px] py-[1px] text-caption text-text-muted dark:bg-neutral-dark dark:text-text-muted-dark"
                  >
                    自动
                  </span>
                </UiTooltip>
              </div>
              <div
                class="mt-[2px] truncate font-mono text-caption text-text-muted dark:text-text-muted-dark"
              >
                {{ listenText(config) }}
                <span class="mx-[4px]">→</span>
                {{ targetText(config) }}
              </div>
              <div
                v-if="runtimeOf(config)?.status === 'error'"
                class="mt-[2px] truncate text-caption text-danger-strong dark:text-danger-dark"
              >
                {{ runtimeOf(config)?.error }}
              </div>
            </div>
            <!-- 连接数 -->
            <UiTooltip
              v-if="runtimeOf(config)?.connections"
              :content="`活动连接 ${runtimeOf(config)?.connections}`"
            >
              <span
                class="shrink-0 font-mono text-caption text-text-muted dark:text-text-muted-dark"
              >
                {{ runtimeOf(config)?.connections }} 连接
              </span>
            </UiTooltip>
            <!-- 状态 -->
            <span
              class="shrink-0 rounded-full px-[9px] py-[3px] text-caption font-medium"
              :class="STATUS_CLASS[runtimeOf(config)?.status ?? 'stopped']"
            >
              {{ STATUS_LABEL[runtimeOf(config)?.status ?? 'stopped'] }}
            </span>
            <!-- 操作 -->
            <div class="flex shrink-0 items-center gap-[2px]">
              <UiButton
                variant="ghost"
                size="xs"
                class="!h-auto !px-[8px] !py-[3px] text-caption"
                :disabled="busyIds.has(config.id)"
                @click="toggleStart(config)"
              >
                {{
                  runtimeOf(config)?.status === 'running' ||
                  runtimeOf(config)?.status === 'starting'
                    ? '停止'
                    : '启动'
                }}
              </UiButton>
              <UiIconButton label="编辑" size="xs" title="编辑" @click="openEdit(config)">
                <UiIcon name="pencil" :size="13" />
              </UiIconButton>
              <UiIconButton label="删除" size="xs" title="删除" @click="deleteTarget = config">
                <UiIcon name="trash" :size="13" />
              </UiIconButton>
            </div>
          </div>
        </div>
      </div>
    </UiScrollArea>

    <!-- 新建/编辑弹窗 -->
    <UiModal
      :open="formOpen"
      :title="editingId ? '编辑隧道' : '新建隧道'"
      width="min(460px, 92vw)"
      @close="formOpen = false"
    >
      <div class="space-y-[10px]">
        <UiField label="名称" required>
          <UiInput v-model="form.name" placeholder="如：生产数据库转发" />
        </UiField>
        <UiField label="类型">
          <Select v-model="form.tunnelType" :options="TYPE_OPTIONS" />
        </UiField>
        <div class="grid grid-cols-2 gap-[10px]">
          <UiField
            label="监听地址"
            :hint="form.tunnelType === 'remote' ? '在服务端监听' : '在本机监听'"
          >
            <UiInput v-model="form.listenHost" class="font-mono" placeholder="127.0.0.1" />
          </UiField>
          <UiField label="监听端口" required>
            <UiInput v-model.number="form.listenPort" type="number" />
          </UiField>
        </div>
        <template v-if="form.tunnelType !== 'dynamic'">
          <UiField
            :label="form.tunnelType === 'remote' ? '本机侧目标地址' : '目标地址（经服务器访问）'"
            required
          >
            <UiInput v-model="form.targetHost" class="font-mono" placeholder="127.0.0.1" />
          </UiField>
          <UiField :label="form.tunnelType === 'remote' ? '本机侧目标端口' : '目标端口'" required>
            <UiInput v-model.number="form.targetPort" type="number" />
          </UiField>
        </template>
        <p
          v-if="form.tunnelType === 'dynamic'"
          class="text-caption text-text-muted dark:text-text-muted-dark"
        >
          本机 {{ listenText({ ...form, id: '', profileId: '' } as TunnelConfig) }} 将作为 SOCKS5
          代理，目标由访问方决定。
        </p>

        <label
          class="flex cursor-pointer items-center gap-[8px] text-body-sm text-secondary dark:text-secondary-dark"
        >
          <UiCheckbox v-model="form.autoStart" />
          连接建立后自动启动此隧道
        </label>

        <template v-if="isDangerListen">
          <p class="text-caption text-danger-strong dark:text-danger-dark">
            监听 0.0.0.0/:: 会把端口暴露给整个网络，存在被第三方访问的风险。输入主机名「{{
              profile?.host
            }}」确认。
          </p>
          <UiInput v-model="dangerConfirmText" class="font-mono" :placeholder="profile?.host" />
        </template>
      </div>
      <template #footer>
        <UiButton variant="ghost" @click="formOpen = false">取消</UiButton>
        <UiButton variant="primary" :disabled="isDangerListen && !canSave" @click="save"
          >保存</UiButton
        >
      </template>
    </UiModal>

    <ConfirmDialog
      :open="deleteTarget !== null"
      title="删除隧道"
      :message="`确定删除隧道「${deleteTarget?.name ?? ''}」？运行中会先停止；配置不可恢复。`"
      confirm-label="删除"
      danger
      @close="deleteTarget = null"
      @confirm="deleteTarget && removeTunnel(deleteTarget)"
    />
  </div>
</template>
