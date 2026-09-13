<script setup lang="ts">
/** SSH 工具主容器：服务器配置列表 + 多连接页签；每条连接拥有完整运维功能区。 */
import { computed, ref, watch } from 'vue'
import ServerList from './profiles/ServerList.vue'
import ServerForm from './profiles/ServerForm.vue'
import TerminalTab from './terminal/TerminalTab.vue'
import TunnelTab from './tunnels/TunnelTab.vue'
import HostKeyDialog from './connection/HostKeyDialog.vue'
import KnownHostsDialog from './connection/KnownHostsDialog.vue'
import FileManagerTab from './files/FileManagerTab.vue'
import MonitorTab from './monitor/MonitorTab.vue'
import ServiceTab from './monitor/ServiceTab.vue'
import ProcessTab from './monitor/ProcessTab.vue'
import DockerTab from './docker/DockerTab.vue'
import {
  useSshWorkspace,
  type SshConnectionWorkspace,
  type SshWorkspaceSection,
} from './useSshWorkspace'
import { UiIcon, UiIconButton, UiTabs, type UiTabItem } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { useSshToolLifecycle } from './toolLifecycle'

const workspace = useSshWorkspace()
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

const sectionTabs: UiTabItem[] = [
  { value: 'terminal', label: '终端' },
  { value: 'files', label: '文件' },
  { value: 'tunnels', label: '隧道' },
  { value: 'monitor', label: '监控' },
  { value: 'services', label: '服务' },
  { value: 'processes', label: '进程' },
  { value: 'docker', label: 'Docker' },
]

const activeWorkspaceId = ref<string | null>(null)
const closingWorkspaceId = ref<string | null>(null)
const openingProfileId = ref<string | null>(null)
/** 「关闭全部会话」确认弹窗开关 */
const cleanupAllOpen = ref(false)
/** 已知主机管理弹窗开关 */
const knownHostsOpen = ref(false)

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
  if (workspace.connection.status === 'disconnected' || workspace.connection.status === 'error') {
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
}

watch(activeWorkspaceId, (id) => {
  const remote = connectionWorkspaces.value.find((item) => item.id === id)
  if (remote) activeProfileId.value = remote.profileId
})

watch(
  () => connectionWorkspaces.value.map((item) => item.id),
  (ids) => {
    if (activeWorkspaceId.value && !ids.includes(activeWorkspaceId.value)) {
      activeWorkspaceId.value = ids[ids.length - 1] ?? null
    }
  }
)
</script>

<template>
  <div class="flex h-full min-h-0 w-full">
    <ServerList
      :profiles="filteredProfiles"
      :groups="groups"
      :expanded-ids="expandedIds"
      :search-keyword="searchKeyword"
      @update:search-keyword="searchKeyword = $event"
      @open-connection="createConnection"
      @add="workspace.openAddForm"
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
        />
        <!-- 一键清理：断开并关闭全部会话 -->
        <UiIconButton
          label="关闭全部会话"
          size="xs"
          class="mx-[6px] shrink-0"
          title="关闭全部会话"
          @click="cleanupAllOpen = true"
        >
          <UiIcon name="trash" :size="13" />
        </UiIconButton>
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
                ? '每个连接页签都包含终端、文件、监控、服务、进程与 Docker。'
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
          <UiTabs
            :model-value="remote.activeSection"
            :items="sectionTabs"
            variant="line"
            @update:model-value="selectSection(remote, $event)"
          />
          <div class="min-h-0 flex-1 overflow-hidden">
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
            />
            <FileManagerTab
              v-if="
                remote.connection.status === 'connected' && remote.visitedSections.includes('files')
              "
              v-show="remote.activeSection === 'files'"
              :connection="remote.connection"
              :profile="profiles.find((profile) => profile.id === remote.profileId)"
              :active="activeWorkspaceId === remote.id && remote.activeSection === 'files'"
              class="h-full"
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
              v-if="remote.connection.status === 'connected' && remote.activeSection === 'monitor'"
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
          </div>
        </div>
      </template>
    </div>

    <ServerForm
      v-if="formOpen"
      :profile="editingProfile"
      @save="(p, creds, saveCredential) => workspace.saveProfile(p, creds, saveCredential)"
      @error="workspace.showError"
      @cancel="formOpen = false"
    />

    <ConfirmDialog
      :open="closingWorkspaceId !== null"
      title="关闭 SSH 连接"
      :message="`确定关闭连接「${closingWorkspace?.title ?? ''}」吗？该连接下的终端、文件传输、监控与日志任务都会结束。`"
      confirm-label="关闭连接"
      @close="closingWorkspaceId = null"
      @confirm="confirmCloseWorkspace"
    />
    <ConfirmDialog
      :open="deleteTarget !== null"
      title="删除服务器连接信息"
      :message="`确定删除「${deleteTarget?.name ?? ''}」（${deleteTarget?.host ?? ''}）的连接信息？该服务器已打开的全部连接也会关闭。`"
      confirm-label="删除"
      danger
      @close="deleteTarget = null"
      @confirm="workspace.confirmDelete"
    />
    <HostKeyDialog :request="hostKeyRequest" @respond="workspace.respondHostKey" />
    <KnownHostsDialog :open="knownHostsOpen" @close="knownHostsOpen = false" />

    <ConfirmDialog
      :open="cleanupAllOpen"
      title="关闭全部会话"
      :message="`将断开并关闭全部 ${connectionWorkspaces.length} 个会话，未保存的终端内容将丢失。`"
      confirm-label="全部关闭"
      danger
      @close="cleanupAllOpen = false"
      @confirm="confirmCleanupAll"
    />
  </div>
</template>
