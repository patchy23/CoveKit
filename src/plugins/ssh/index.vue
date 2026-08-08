<script setup lang="ts">
/**
 * SSH 工具 · 主容器（服务器列表 + 页签工作区）
 * 当前为纯前端 UI 演示，后端 IPC 接入后替换 mock 数据与本地状态。
 */
import { computed, ref } from "vue";
import { useUiStore } from "@/stores/ui";
import ServerList from "./ServerList.vue";
import ServerForm from "./ServerForm.vue";
import TerminalTab from "./TerminalTab.vue";
import FileManagerTab from "./FileManagerTab.vue";
import MonitorTab from "./MonitorTab.vue";
import ServiceTab from "./ServiceTab.vue";
import ProcessTab from "./ProcessTab.vue";
import DockerTab from "./DockerTab.vue";
import EditorTab from "./EditorTab.vue";
import {
  mockConnections,
  mockProfiles,
  mockTerminals,
  statusDotClass,
  statusText,
} from "./useSsh";
import type { ServerProfile } from "./contracts";

const ui = useUiStore();

/* ── 服务器列表状态 ── */
const profiles = ref<ServerProfile[]>([...mockProfiles]);
const connections = ref([...mockConnections]);
const activeProfileId = ref<string | null>("profile-1");
const searchKeyword = ref("");

const filteredProfiles = computed(() => {
  const kw = searchKeyword.value.trim().toLowerCase();
  if (!kw) return profiles.value;
  return profiles.value.filter(
    (p) =>
      p.name.toLowerCase().includes(kw) ||
      p.host.toLowerCase().includes(kw) ||
      p.username.toLowerCase().includes(kw),
  );
});

const activeConnection = computed(() =>
  connections.value.find((c) => c.profileId === activeProfileId.value),
);

/* ── 服务器表单弹窗 ── */
const formOpen = ref(false);
const editingProfile = ref<ServerProfile | null>(null);

function openAddForm() {
  editingProfile.value = null;
  formOpen.value = true;
}

function openEditForm(p: ServerProfile) {
  editingProfile.value = { ...p };
  formOpen.value = true;
}

function saveProfile(p: ServerProfile) {
  const idx = profiles.value.findIndex((x) => x.id === p.id);
  if (idx >= 0) {
    profiles.value[idx] = p;
    ui.toast(`已更新服务器「${p.name}」`);
  } else {
    profiles.value.push(p);
    ui.toast(`已添加服务器「${p.name}」`);
  }
  formOpen.value = false;
}

function deleteProfile(id: string) {
  const p = profiles.value.find((x) => x.id === id);
  if (!p) return;
  profiles.value = profiles.value.filter((x) => x.id !== id);
  connections.value = connections.value.filter((x) => x.profileId !== id);
  if (activeProfileId.value === id) activeProfileId.value = null;
  ui.toast(`已删除服务器「${p.name}」`);
}

/* ── 连接操作 ── */
function connect(profileId: string) {
  const conn = connections.value.find((c) => c.profileId === profileId);
  if (conn) {
    conn.status = "connecting";
    setTimeout(() => {
      conn.status = "connected";
      conn.host = profiles.value.find((p) => p.id === profileId)?.host;
      conn.latencyMs = Math.floor(Math.random() * 50) + 10;
      conn.connectedAt = Date.now();
      ui.toast(`已连接到 ${conn.host}`);
    }, 800);
  }
  activeProfileId.value = profileId;
}

function disconnect(profileId: string) {
  const conn = connections.value.find((c) => c.profileId === profileId);
  if (conn) {
    conn.status = "disconnected";
    conn.host = undefined;
    conn.latencyMs = undefined;
    conn.connectedAt = undefined;
    ui.toast("已断开连接");
  }
}

/* ── 页签工作区 ── */
const tabs = [
  { id: "terminal", name: "终端", component: TerminalTab },
  { id: "files", name: "文件", component: FileManagerTab },
  { id: "monitor", name: "监控", component: MonitorTab },
  { id: "services", name: "服务", component: ServiceTab },
  { id: "processes", name: "进程", component: ProcessTab },
  { id: "docker", name: "Docker", component: DockerTab },
  { id: "editor", name: "编辑", component: EditorTab },
] as const;

const activeTabId = ref<string>("terminal");

const activeTab = computed(() => tabs.find((t) => t.id === activeTabId.value) ?? tabs[0]);
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
      @connect="connect"
      @disconnect="disconnect"
      @add="openAddForm"
      @edit="openEditForm"
      @delete="deleteProfile"
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
          <div class="ml-auto flex gap-[6px]">
            <button
              v-if="activeConnection?.status === 'connected'"
              class="btn-ghost text-body-sm"
              @click="disconnect(activeProfileId)"
            >
              断开
            </button>
            <button
              v-else
              class="btn-primary !h-[30px] !px-[12px] text-body-sm"
              @click="connect(activeProfileId)"
            >
              连接
            </button>
          </div>
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
            :is="activeTab.component"
            :connection="activeConnection"
            :profile="profiles.find((p) => p.id === activeProfileId)"
            :terminals="mockTerminals"
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
      @cancel="formOpen = false"
    />
  </div>
</template>
