<script setup lang="ts">
/**
 * SSH 工具 · 主容器（服务器列表 + 页签工作区）
 * 连接/断开/凭证走真实 IPC；状态经事件 ssh://connection-status 同步。
 */
import { ref } from "vue";
import ServerList from "./ServerList.vue";
import ServerForm from "./ServerForm.vue";
import TerminalTab from "./TerminalTab.vue";
import FileManagerTab from "./FileManagerTab.vue";
import MonitorTab from "./MonitorTab.vue";
import ServiceTab from "./ServiceTab.vue";
import ProcessTab from "./ProcessTab.vue";
import DockerTab from "./DockerTab.vue";
import { statusDotClass, statusText } from "./useSsh";
import { useSshWorkspace } from "./useSshWorkspace";

const {
  profiles,
  connections,
  activeProfileId,
  searchKeyword,
  filteredProfiles,
  activeConnection,
  usableConnection,
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
} = useSshWorkspace();

const tabs = [
  { id: "terminal", name: "终端", component: TerminalTab },
  { id: "files", name: "文件", component: FileManagerTab },
  { id: "monitor", name: "监控", component: MonitorTab },
  { id: "services", name: "服务", component: ServiceTab },
  { id: "processes", name: "进程", component: ProcessTab },
  { id: "docker", name: "Docker", component: DockerTab },
] as const;

const activeTabId = ref<string>("terminal");
const terminalConnectRequest = ref(0);

/** 左侧服务器连接属于显式操作：连接成功后同步打开该服务器终端。 */
async function connectAndOpenTerminal(profileId: string) {
  activeTabId.value = "terminal";
  await connect(profileId);
  terminalConnectRequest.value += 1;
}
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
      @select="activeProfileId = $event"
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
            {{ profiles.find((p) => p.id === activeProfileId)?.name }}
          </span>
          <span class="font-mono text-body-sm text-text-muted dark:text-text-muted-dark">
            {{ activeConnection?.host ?? "—" }}
          </span>
          <span class="text-caption text-text-muted dark:text-text-muted-dark">
            {{ statusText(activeConnection?.status ?? "disconnected") }}
            <template v-if="activeConnection?.latencyMs">
              · {{ activeConnection.latencyMs }}ms
            </template>
          </span>
        </div>

        <!-- 子页签条 -->
        <div
          class="flex shrink-0 items-center gap-[2px] border-b border-border px-[8px] dark:border-border-dark"
        >
          <button
            v-for="t in tabs"
            :key="t.id"
            class="relative rounded-t-md px-[14px] py-[8px] text-body font-medium transition-colors"
            :class="
              activeTabId === t.id
                ? 'text-tertiary-strong dark:text-tertiary-dark'
                : 'text-secondary hover:text-primary dark:text-secondary-dark dark:hover:text-primary-dark'
            "
            @click="activeTabId = t.id"
          >
            {{ t.name }}
            <span
              v-if="activeTabId === t.id"
              class="absolute inset-x-0 top-0 h-[2px] bg-tertiary-strong dark:bg-tertiary-dark"
            />
          </button>
        </div>

        <!-- 页签内容区 -->
        <div class="min-h-0 flex-1 overflow-hidden">
          <component
            :is="tab.component"
            v-for="tab in tabs"
            v-show="activeTabId === tab.id"
            :key="tab.id"
            :connection="usableConnection"
            :profile="profiles.find((p) => p.id === activeProfileId)"
            v-bind="tab.id === 'terminal' ? { connectRequest: terminalConnectRequest } : {}"
            class="h-full"
          />
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
      <div
        v-if="deleteTarget"
        class="fixed inset-0 z-[160] grid place-items-center bg-black/30"
        @click.self="deleteTarget = null"
      >
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
