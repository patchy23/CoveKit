<script setup lang="ts">
/** SSH 工具主容器：服务器配置列表 + 多连接页签；每条连接拥有完整运维功能区。 */
import { computed, defineAsyncComponent, h, ref, watch, type Component } from 'vue'
import { useUiStore } from '@/stores/ui'
import LiveLogDialog from './monitor/LiveLogDialog.vue'
import { provideLogWindows } from './monitor/logWindows'
import ServerList from './profiles/ServerList.vue'
import ServerForm from './profiles/ServerForm.vue'
import ServerImportDialog from './profiles/ServerImportDialog.vue'
import ServerExportDialog from './profiles/ServerExportDialog.vue'
import ConnectionCredentialsDialog from './connection/ConnectionCredentialsDialog.vue'
import HostKeyDialog from './connection/HostKeyDialog.vue'
import KnownHostsDialog from './connection/KnownHostsDialog.vue'
import {
  useSshWorkspace,
  type SshConnectionWorkspace,
  type SshWorkspaceSection,
} from './useSshWorkspace'
import { UiSpinner, UiTabs, type UiTabItem } from '@/core/ui'
import { UiConfirmDialog } from '@/core/ui'
import { UiContextMenu, type UiContextMenuItem } from '@/core/ui'
import { useSshToolLifecycle } from './toolLifecycle'

/** 功能页首次进入才加载；加载异常继续交给 ToolHost 的错误面板和重试入口。 */
function lazySection<T extends Component>(loader: () => Promise<{ default: T }>) {
  return defineAsyncComponent({
    loader,
    delay: 120,
    timeout: 30000,
    loadingComponent: () =>
      h('div', { class: 'grid h-full place-items-center' }, [
        h(UiSpinner, { label: '正在加载功能页' }),
      ]),
  })
}

const TerminalTab = lazySection(() => import('./terminal/TerminalTab.vue'))
const ConnectionEditor = lazySection(() => import('./files/ConnectionEditor.vue'))
const FileManagerTab = lazySection(() => import('./files/FileManagerTab.vue'))
const TunnelTab = lazySection(() => import('./tunnels/TunnelTab.vue'))
const MonitorTab = lazySection(() => import('./monitor/MonitorTab.vue'))
const ServiceTab = lazySection(() => import('./monitor/ServiceTab.vue'))
const ProcessTab = lazySection(() => import('./monitor/ProcessTab.vue'))
const DockerTab = lazySection(() => import('./docker/DockerTab.vue'))
const ComposeTab = lazySection(() => import('./compose/ComposeTab.vue'))

const bulk = ref<{ mode: 'import' | 'export'; groupId?: string }>()
const workspace = useSshWorkspace()
const ui = useUiStore()
// 工具资源生命周期：关闭页签/退出时断开会话与隧道（T10-4）
useSshToolLifecycle()
const {
  profiles,
  groups,
  expandedIds,
  connectionWorkspaces,
  activeProfileId,
  searchKeyword,
  filteredProfiles,
  formOpen,
  editingProfile,
  deleteTarget,
  hostKeyRequest,
} = workspace

const { windows: logWindows, close: closeLogWindow } = provideLogWindows(() =>
  connectionWorkspaces.value
    .filter((item) => item.connection.status === 'connected')
    .map((item) => ({ sessionId: item.connection.sessionId, title: item.title }))
)

const sectionTabs: UiTabItem[] = [
  { value: 'terminal', label: '终端' },
  { value: 'files', label: '文件' },
  { value: 'tunnels', label: '隧道' },
  { value: 'monitor', label: '监控' },
  { value: 'services', label: '服务' },
  { value: 'processes', label: '进程' },
  { value: 'docker', label: '容器' },
  { value: 'compose', label: '编排' },
]

const activeWorkspaceId = ref<string | null>(null)
const closingWorkspaceId = ref<string | null>(null)
const editorMinimized = ref<Record<string, boolean>>({})
const editorRequests = ref<Record<string, { id: number; path?: string }>>({})
const editorRenames = ref<Record<string, { oldPath: string; newPath: string }>>({})
const fileDirectories = ref<Record<string, string>>({})
const fileNavigations = ref<Record<string, { id: number; sessionId: string; path: string }>>({})
let requestId = 0
function returnEditor(id: string) {
  ui.openTool('ssh')
  activeWorkspaceId.value = id
}
function openEditor(id: string, path?: string) {
  editorRequests.value[id] = { id: ++requestId, path }
}
function viewFiles(remote: SshConnectionWorkspace, request: { sessionId: string; path: string }) {
  if (remote.connection.sessionId !== request.sessionId || activeWorkspaceId.value !== remote.id)
    return
  fileNavigations.value[remote.id] = { ...request, id: ++requestId }
  selectSection(remote, 'files')
}
const fileStates = ref<Record<string, { dirty: boolean; busy: boolean }>>({})
const composeStates = ref<Record<string, { dirty: boolean; busy: boolean }>>({})
const composeCloseHint = computed(() => {
  const state = closingWorkspaceId.value ? composeStates.value[closingWorkspaceId.value] : undefined
  const file = closingWorkspaceId.value ? fileStates.value[closingWorkspaceId.value] : undefined
  return (
    (file?.dirty || file?.busy ? '远程文件有未保存内容或正在读写，关闭将丢失草稿。' : '') +
    `${state?.dirty ? '编排页有未保存的 YAML，关闭将丢失修改。' : ''}${state?.busy ? '编排操作仍在执行，关闭后远端操作不保证停止。' : ''}`
  )
})
const openingProfileId = ref<string | null>(null)
/** 「关闭全部会话」确认弹窗开关 */
const cleanupAllOpen = ref(false)
const connectionMenu = ref<{ x: number; y: number } | null>(null)
const connectionMenuItems: UiContextMenuItem[] = [
  { label: '关闭全部', onClick: () => (cleanupAllOpen.value = true) },
]

function openConnectionMenu(id: string, event: MouseEvent) {
  event.preventDefault()
  if (!connectionWorkspaces.value.some((item) => item.id === id)) return
  connectionMenu.value = {
    x: event.clientX,
    y: event.clientY,
  }
}
/** 已知主机管理弹窗开关 */
const knownHostsOpen = ref(false)
const defaultGroupId = ref<string | null>(null)
function openAddServer(groupId?: string) {
  defaultGroupId.value = groupId ?? null
  workspace.openAddForm()
}

/** 确认清理：断开并关闭全部连接工作区 */
async function confirmCleanupAll() {
  cleanupAllOpen.value = false
  activeWorkspaceId.value = null
  await workspace.closeAllWorkspaces()
}
const activeWorkspace = computed(() =>
  connectionWorkspaces.value.find((item) => item.id === activeWorkspaceId.value)
)
const activeProfile = computed(() =>
  profiles.value.find(
    (item) => item.id === (activeWorkspace.value?.profileId ?? activeProfileId.value)
  )
)
const connectionTabItems = computed<UiTabItem[]>(() =>
  connectionWorkspaces.value.map((item) => ({
    value: item.id,
    label: item.title,
    closable: true,
    status:
      item.connection.status === 'connected'
        ? ('success' as const)
        : item.connection.status === 'connecting' || item.connection.status === 'reconnecting'
          ? ('progress' as const)
          : ('danger' as const),
  }))
)
const closingWorkspace = computed(() =>
  connectionWorkspaces.value.find((item) => item.id === closingWorkspaceId.value)
)

async function createConnection(profileId: string) {
  if (openingProfileId.value) return
  openingProfileId.value = profileId
  const remote = await workspace.openConnection(profileId)
  openingProfileId.value = null
  if (remote) activeWorkspaceId.value = remote.id
}

function requestCloseWorkspace(id: string) {
  const workspace = connectionWorkspaces.value.find((item) => item.id === id)
  if (!workspace) return
  // 已断开（或连接失败）的页签直接关闭，无需确认；活连接才提醒会话将结束
  if (
    (workspace.connection.status === 'disconnected' || workspace.connection.status === 'error') &&
    !composeStates.value[id]?.dirty &&
    !composeStates.value[id]?.busy &&
    !fileStates.value[id]?.dirty &&
    !fileStates.value[id]?.busy
  ) {
    void confirmCloseWorkspace(id)
    return
  }
  closingWorkspaceId.value = id
}

function selectSection(remote: SshConnectionWorkspace, section: string) {
  const next = section as SshWorkspaceSection
  remote.activeSection = next
  if (!remote.visitedSections.includes(next)) remote.visitedSections.push(next)
  workspace.touchWorkspace(remote.id)
}

async function confirmCloseWorkspace(passedId?: string) {
  const id = passedId ?? closingWorkspaceId.value
  closingWorkspaceId.value = null
  if (!id) return
  const index = connectionWorkspaces.value.findIndex((item) => item.id === id)
  if (activeWorkspaceId.value === id) {
    activeWorkspaceId.value =
      connectionWorkspaces.value[index + 1]?.id ?? connectionWorkspaces.value[index - 1]?.id ?? null
  }
  await workspace.closeConnectionWorkspace(id)
  delete editorRequests.value[id]
  delete editorMinimized.value[id]
  delete editorRenames.value[id]
  delete fileDirectories.value[id]
  delete fileNavigations.value[id]
  delete fileStates.value[id]
  delete composeStates.value[id]
}

watch(activeWorkspaceId, (id) => {
  const remote = connectionWorkspaces.value.find((item) => item.id === id)
  if (remote) activeProfileId.value = remote.profileId
})

watch(
  () => connectionWorkspaces.value.map((item) => item.id),
  (ids) => {
    connectionMenu.value = null
    for (const record of [
      editorRequests,
      editorMinimized,
      editorRenames,
      fileDirectories,
      fileNavigations,
      fileStates,
      composeStates,
    ]) {
      for (const id of Object.keys(record.value)) if (!ids.includes(id)) delete record.value[id]
    }
    if (activeWorkspaceId.value && !ids.includes(activeWorkspaceId.value)) {
      activeWorkspaceId.value = ids[ids.length - 1] ?? null
    }
  }
)
</script>

<template>
  <div class="relative isolate flex h-full min-h-0 w-full">
    <ServerList
      :busy="workspace.treeMoving.value"
      :profiles="filteredProfiles"
      :groups="groups"
      :expanded-ids="expandedIds"
      :search-keyword="searchKeyword"
      @tree-move="workspace.moveTree"
      @update:search-keyword="searchKeyword = $event"
      @open-connection="createConnection"
      @add="openAddServer"
      @bulk-import="(groupId) => (bulk = { mode: 'import', groupId })"
      @bulk-export="(groupId) => (bulk = { mode: 'export', groupId })"
      @known-hosts="knownHostsOpen = true"
      @edit="workspace.openEditForm"
      @delete-request="workspace.requestDelete"
      @move-to-group="workspace.moveToGroup"
      @toggle-group="workspace.toggleGroup"
      @create-group="workspace.createGroup"
      @rename-group="workspace.renameGroup"
      @delete-group="workspace.deleteGroup"
    />

    <div class="flex min-h-0 min-w-0 flex-1 flex-col">
      <!-- 第一层：完整 SSH 连接页签。关闭它才释放连接及其全部任务。 -->
      <div v-if="connectionTabItems.length" class="flex shrink-0 items-center">
        <UiTabs
          :model-value="activeWorkspaceId ?? ''"
          :items="connectionTabItems"
          variant="line"
          size="sm"
          class="min-w-0 flex-1"
          @update:model-value="activeWorkspaceId = $event"
          @close="requestCloseWorkspace"
          @contextmenu="openConnectionMenu"
        />
      </div>

      <div
        v-if="!activeWorkspace"
        class="grid flex-1 place-items-center text-text-muted dark:text-text-muted-dark"
      >
        <div class="text-center">
          <p class="mb-[8px] text-h2">
            {{ activeProfile ? '双击服务器建立连接' : '选择或添加一台服务器' }}
          </p>
          <p class="text-body-sm">
            {{
              activeProfile
                ? '每个连接页签都包含终端、文件、监控、服务、进程、容器与编排。'
                : '从左侧选择服务器配置。'
            }}
          </p>
        </div>
      </div>

      <template v-for="remote in connectionWorkspaces" :key="remote.id">
        <div
          v-show="activeWorkspaceId === remote.id"
          class="flex min-h-0 flex-1 flex-col"
          @mousedown.capture="workspace.touchWorkspace(remote.id)"
        >
          <!-- 第二层：当前连接内部的功能页签。 -->
          <div class="flex shrink-0 items-center border-b border-border dark:border-border-dark">
            <UiTabs
              class="min-w-0 flex-1"
              :model-value="remote.activeSection"
              :items="sectionTabs"
              variant="line"
              @update:model-value="selectSection(remote, $event)"
            />
            <button
              type="button"
              data-ssh-editor-action
              class="mx-sm shrink-0 rounded-sm px-sm py-xs text-body-sm text-secondary hover:bg-surface-muted hover:text-primary focus-visible:outline focus-visible:outline-2 focus-visible:outline-tertiary disabled:opacity-40 dark:text-secondary-dark dark:hover:bg-surface-muted-dark dark:hover:text-primary-dark"
              :disabled="remote.connection.status !== 'connected' && !editorRequests[remote.id]"
              @click="openEditor(remote.id)"
            >
              {{ editorMinimized[remote.id] ? '恢复编辑器' : '打开编辑器' }}
            </button>
          </div>
          <div class="relative min-h-0 flex-1 overflow-hidden">
            <TerminalTab
              v-if="remote.visitedSections.includes('terminal')"
              v-show="remote.activeSection === 'terminal'"
              :connection="remote.connection"
              :profile="profiles.find((profile) => profile.id === remote.profileId)"
              :connect-request="remote.connectRequest"
              :reconnect-tick="remote.reconnectTick"
              :stage-text="remote.stageText"
              :active="activeWorkspaceId === remote.id && remote.activeSection === 'terminal'"
              class="h-full"
              @reconnect="workspace.reconnectWorkspace(remote.id)"
              @link-dead="workspace.handleLinkDead(remote.id)"
              @view-files="viewFiles(remote, $event)"
            />
            <FileManagerTab
              v-if="remote.visitedSections.includes('files')"
              v-show="remote.activeSection === 'files'"
              :connection="remote.connection"
              :profile="profiles.find((profile) => profile.id === remote.profileId)"
              :active="activeWorkspaceId === remote.id && remote.activeSection === 'files'"
              class="h-full"
              :navigation="fileNavigations[remote.id]"
              @open-file="openEditor(remote.id, $event.path)"
              @directory="fileDirectories[remote.id] = $event"
              @renamed="(oldPath, newPath) => (editorRenames[remote.id] = { oldPath, newPath })"
            />
            <TunnelTab
              v-if="
                remote.connection.status === 'connected' &&
                remote.visitedSections.includes('tunnels')
              "
              v-show="remote.activeSection === 'tunnels'"
              :connection="remote.connection"
              :profile="profiles.find((profile) => profile.id === remote.profileId)"
              class="h-full"
            />
            <MonitorTab
              v-if="
                remote.connection.status === 'connected' &&
                remote.visitedSections.includes('monitor')
              "
              v-show="remote.activeSection === 'monitor'"
              :active="activeWorkspaceId === remote.id && remote.activeSection === 'monitor'"
              :connection="remote.connection"
              :profile="profiles.find((profile) => profile.id === remote.profileId)"
              class="h-full"
            />
            <ServiceTab
              v-if="remote.connection.status === 'connected' && remote.activeSection === 'services'"
              :connection="remote.connection"
              :profile="profiles.find((profile) => profile.id === remote.profileId)"
              class="h-full"
            />
            <ProcessTab
              v-if="
                remote.connection.status === 'connected' && remote.activeSection === 'processes'
              "
              :connection="remote.connection"
              :profile="profiles.find((profile) => profile.id === remote.profileId)"
              class="h-full"
            />
            <DockerTab
              v-if="remote.connection.status === 'connected' && remote.activeSection === 'docker'"
              :connection="remote.connection"
              :profile="profiles.find((profile) => profile.id === remote.profileId)"
              class="h-full"
            />
            <ComposeTab
              v-if="remote.visitedSections.includes('compose')"
              v-show="remote.activeSection === 'compose'"
              :connection="remote.connection"
              :profile-id="remote.profileId"
              :workspace-id="remote.id"
              class="h-full"
              @state="composeStates[remote.id] = $event"
            />
          </div>
        </div>
      </template>
    </div>

    <!-- 与日志窗口共用 SSH 工具根宿主，不受功能页的裁剪与连接页签隐藏影响。 -->
    <template v-for="remote in connectionWorkspaces" :key="'editor-' + remote.id">
      <ConnectionEditor
        v-if="editorRequests[remote.id]"
        :connection="remote.connection"
        :title="remote.title"
        :request="editorRequests[remote.id]"
        :rename="editorRenames[remote.id]"
        :directory="fileDirectories[remote.id]"
        @state="fileStates[remote.id] = $event"
        @minimized="editorMinimized[remote.id] = $event"
        @returned="returnEditor(remote.id)"
      />
    </template>

    <LiveLogDialog
      v-for="log in logWindows"
      :key="log.key"
      :title="log.title"
      :connection-id="log.connectionId"
      :kind="log.kind"
      :target-id="log.targetId"
      :activation="log.activation"
      @close="closeLogWindow(log.key)"
    />

    <UiContextMenu
      v-if="connectionMenu"
      :x="connectionMenu.x"
      :y="connectionMenu.y"
      :items="connectionMenuItems"
      size="sm"
      @close="connectionMenu = null"
    />

    <ConnectionCredentialsDialog
      v-if="workspace.credentialRequestProfile.value"
      :key="workspace.credentialRequestProfile.value.id"
      :profile="workspace.credentialRequestProfile.value"
      @confirm="workspace.respondCredentials"
      @cancel="workspace.respondCredentials()"
    />
    <ServerImportDialog
      v-if="bulk?.mode === 'import'"
      :profiles="profiles"
      :groups="groups"
      :group-id="bulk.groupId"
      @close="bulk = undefined"
      @changed="workspace.reloadProfiles"
    />
    <ServerExportDialog
      v-if="bulk?.mode === 'export'"
      :profiles="profiles"
      :groups="groups"
      :group-id="bulk.groupId"
      @close="bulk = undefined"
    />
    <ServerForm
      v-if="formOpen"
      :profile="editingProfile"
      :groups="groups"
      :default-group-id="defaultGroupId"
      @save="
        (p, creds, saveCredential, saveLocal) =>
          workspace.saveProfile(p, creds, saveCredential, saveLocal)
      "
      @error="workspace.showError"
      @cancel="formOpen = false"
    />

    <UiConfirmDialog
      :open="closingWorkspaceId !== null"
      title="关闭 SSH 连接"
      :message="`确定关闭连接「${closingWorkspace?.title ?? ''}」吗？该连接下的终端、文件传输、监控与日志任务都会结束。${composeCloseHint}`"
      confirm-label="关闭连接"
      @close="closingWorkspaceId = null"
      @confirm="confirmCloseWorkspace"
    />
    <UiConfirmDialog
      :open="deleteTarget !== null"
      title="删除服务器连接信息"
      :message="`确定删除「${deleteTarget?.name ?? ''}」（${deleteTarget?.host ?? ''}）的连接信息？该服务器已打开的全部连接也会关闭，未保存的编排配置将丢失，正在执行的远端操作不保证停止。`"
      confirm-label="删除"
      danger
      @close="deleteTarget = null"
      @confirm="workspace.confirmDelete"
    />
    <HostKeyDialog :request="hostKeyRequest" @respond="workspace.respondHostKey" />
    <KnownHostsDialog :open="knownHostsOpen" @close="knownHostsOpen = false" />

    <UiConfirmDialog
      :open="cleanupAllOpen"
      title="关闭全部会话"
      :message="`将断开并关闭全部 ${connectionWorkspaces.length} 个会话，未保存的远程文件、终端内容及编排配置将丢失；正在执行的远端操作不保证停止。`"
      confirm-label="全部关闭"
      danger
      @close="cleanupAllOpen = false"
      @confirm="confirmCleanupAll"
    />
  </div>
</template>
