<script setup lang="ts">
/**
 * SSH 工具 · 主容器（服务器列表 + 页签工作区）
 * 连接/断开/凭证走真实 IPC；状态经事件 ssh://connection-status 同步。
 */
import { computed, ref, watch } from 'vue'
import ServerList from './ServerList.vue'
import ServerForm from './ServerForm.vue'
import TerminalTab from './TerminalTab.vue'
import FileManagerTab from './FileManagerTab.vue'
import MonitorTab from './MonitorTab.vue'
import ServiceTab from './ServiceTab.vue'
import ProcessTab from './ProcessTab.vue'
import DockerTab from './DockerTab.vue'
import { statusDotClass, statusText } from './useSsh'
import { useSshWorkspace } from './useSshWorkspace'
import { UiTabs } from '@/core/ui'

const {
  profiles,
  connections,
  activeProfileId,
  searchKeyword,
  filteredProfiles,
  activeConnection,
  formOpen,
  editingProfile,
  deleteTarget,
  openAddForm,
  openEditForm,
  showError,
  saveProfile,
  requestDelete,
  confirmDelete,
  connect,
  disconnect,
} = useSshWorkspace()

const tabs = [
  { id: 'terminal', name: '终端', component: TerminalTab },
  { id: 'files', name: '文件', component: FileManagerTab },
  { id: 'monitor', name: '监控', component: MonitorTab },
  { id: 'services', name: '服务', component: ServiceTab },
  { id: 'processes', name: '进程', component: ProcessTab },
  { id: 'docker', name: 'Docker', component: DockerTab },
] as const
const tabItems = tabs.map((item) => ({ value: item.id, label: item.name }))

const activeTabId = ref<string>('terminal')
const visitedProfileIds = ref<string[]>([])
const terminalConnectRequests = ref<Record<string, number>>({})
const activeProfile = computed(() =>
  profiles.value.find((item) => item.id === activeProfileId.value)
)
const showConnectionHost = computed(
  () =>
    Boolean(activeConnection.value?.host) &&
    activeConnection.value?.host !== activeProfile.value?.name
)

/** 记录已打开过的服务器工作区，使不同服务器的终端与页签状态独立保活。 */
function selectProfile(profileId: string) {
  activeProfileId.value = profileId
  if (!visitedProfileIds.value.includes(profileId)) visitedProfileIds.value.push(profileId)
}

/** 返回指定服务器当前可用的连接，而不是复用当前选中服务器的连接。 */
function usableConnectionFor(profileId: string) {
  const connection = connections.value.find((item) => item.profileId === profileId)
  return connection?.status === 'connected' ? connection : undefined
}

/** 左侧服务器连接属于显式操作：连接成功后同步打开该服务器终端。 */
async function connectAndOpenTerminal(profileId: string) {
  selectProfile(profileId)
  activeTabId.value = 'terminal'
  await connect(profileId)
  terminalConnectRequests.value = {
    ...terminalConnectRequests.value,
    [profileId]: (terminalConnectRequests.value[profileId] ?? 0) + 1,
  }
}

watch(activeProfileId, (profileId) => {
  if (profileId && !visitedProfileIds.value.includes(profileId)) {
    visitedProfileIds.value.push(profileId)
  }
})
</script>

<template>
  <div class="flex h-full min-h-0 w-full">
    <!-- 左侧：服务器列表 -->
    <ServerList
      :profiles="filteredProfiles"
      :connections="connections"
      :active-profile-id="activeProfileId"
      :search-keyword="searchKeyword"
      @update:search-keyword="searchKeyword = $event"
      @select="selectProfile"
      @connect="connectAndOpenTerminal"
      @disconnect="disconnect"
      @add="openAddForm"
      @edit="openEditForm"
      @delete-request="requestDelete"
    />

    <!-- 右侧：页签工作区 -->
    <div class="flex min-h-0 min-w-0 flex-1 flex-col">
      <!-- 无选中服务器时的空态 -->
      <div
        v-if="!activeProfileId"
        class="grid flex-1 place-items-center text-text-muted dark:text-text-muted-dark"
      >
        <div class="text-center">
          <p class="text-h2 mb-[8px]">选择或添加一台服务器</p>
          <p class="text-body-sm">从左侧列表选择，或点击「+ 添加服务器」新建配置</p>
        </div>
      </div>

      <template v-else>
        <!-- 当前服务器信息条 -->
        <div
          class="flex shrink-0 items-center gap-[10px] border-b border-border px-[16px] py-[10px] dark:border-border-dark"
        >
          <span
            class="inline-block h-[8px] w-[8px] rounded-full"
            :class="statusDotClass(activeConnection?.status ?? 'disconnected')"
          />
          <span class="text-body font-medium text-primary dark:text-primary-dark">
            {{ activeProfile?.name }}
          </span>
          <span
            v-if="showConnectionHost"
            class="font-mono text-body-sm text-text-muted dark:text-text-muted-dark"
          >
            {{ activeConnection?.host ?? '—' }}
          </span>
          <span class="text-caption text-text-muted dark:text-text-muted-dark">
            {{ statusText(activeConnection?.status ?? 'disconnected') }}
            <template v-if="activeConnection?.latencyMs">
              · {{ activeConnection.latencyMs }}ms
            </template>
          </span>
        </div>

        <!-- 子页签条 -->
        <UiTabs v-model="activeTabId" :items="tabItems" variant="line" />

        <!-- 页签内容区 -->
        <div class="min-h-0 flex-1 overflow-hidden">
          <template
            v-for="profile in profiles.filter((item) => visitedProfileIds.includes(item.id))"
            :key="profile.id"
          >
            <component
              :is="tab.component"
              v-for="tab in tabs"
              v-show="activeProfileId === profile.id && activeTabId === tab.id"
              :key="`${profile.id}:${tab.id}`"
              :connection="usableConnectionFor(profile.id)"
              :profile="profile"
              v-bind="
                tab.id === 'terminal'
                  ? { connectRequest: terminalConnectRequests[profile.id] ?? 0 }
                  : {}
              "
              class="h-full"
            />
          </template>
        </div>
      </template>
    </div>

    <!-- 服务器表单弹窗 -->
    <ServerForm
      v-if="formOpen"
      :profile="editingProfile"
      @save="saveProfile"
      @error="showError"
      @cancel="formOpen = false"
    />

    <!-- 删除确认弹窗 -->
    <Teleport to="body">
      <div v-if="deleteTarget" class="fixed inset-0 z-[160] grid place-items-center bg-black/30">
        <div
          class="w-[380px] rounded-lg border border-border bg-surface p-[18px] shadow-[0_16px_48px_rgba(16,24,40,0.25)] dark:border-border-dark dark:bg-surface-dark"
        >
          <h3 class="mb-[8px] text-card-title font-medium text-primary dark:text-primary-dark">
            删除服务器连接信息
          </h3>
          <p class="mb-[16px] text-body text-secondary dark:text-secondary-dark">
            确定删除「{{ deleteTarget.name }}」（{{
              deleteTarget.host
            }}）的连接信息？此操作不可撤销。
          </p>
          <div class="flex justify-end gap-[8px]">
            <button class="btn-ghost" @click="deleteTarget = null">取消</button>
            <button class="btn-primary !bg-danger-strong" @click="confirmDelete">删除</button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>
