<script setup lang="ts">
/**
 * SSH 工具 · 主容器（服务器列表 + 页签工作区）
 * 连接/断开/凭证走真实 IPC；状态经事件 ssh://connection-status 同步。
 */
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useUiStore } from "@/stores/ui";
import ServerList from "./ServerList.vue";
import ServerForm from "./ServerForm.vue";
import TerminalTab from "./TerminalTab.vue";
import FileManagerTab from "./FileManagerTab.vue";
import MonitorTab from "./MonitorTab.vue";
import ServiceTab from "./ServiceTab.vue";
import ProcessTab from "./ProcessTab.vue";
import DockerTab from "./DockerTab.vue";
import { loadProfiles, persistProfiles, statusDotClass, statusText } from "./useSsh";
import { ipc, onConnectionStatus } from "./ipc";
import type { ServerConnection, ServerProfile } from "./contracts";

const ui = useUiStore();

/* ── 服务器列表状态（配置 localStorage 持久化，凭证后端加密存储）── */
const profiles = ref<ServerProfile[]>(loadProfiles());
const connections = ref<ServerConnection[]>([]);
const activeProfileId = ref<string | null>(null);
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

function saveProfile(
  p: ServerProfile,
  creds: { password?: string; privateKey?: string; passphrase?: string },
) {
  const idx = profiles.value.findIndex((x) => x.id === p.id);
  if (idx >= 0) {
    profiles.value[idx] = p;
    ui.toast(`已更新服务器「${p.name}」`);
  } else {
    profiles.value.push(p);
    ui.toast(`已添加服务器「${p.name}」`);
  }
  persistProfiles(profiles.value);
  // 凭证（密码/私钥）加密存储到后端（AES-GCM），仅当用户填写时更新
  if (creds.password || creds.privateKey || creds.passphrase) {
    ipc.sshCredentialSave({ profile: p, ...creds }).catch((e) => ui.toast(`凭证保存失败：${e}`));
  }
  formOpen.value = false;
}

function deleteProfile(id: string) {
  const p = profiles.value.find((x) => x.id === id);
  if (!p) return;
  profiles.value = profiles.value.filter((x) => x.id !== id);
  connections.value = connections.value.filter((x) => x.profileId !== id);
  if (activeProfileId.value === id) activeProfileId.value = null;
  persistProfiles(profiles.value);
  ipc.sshCredentialDelete(id).catch(() => undefined);
  ui.toast(`已删除服务器「${p.name}」`);
}

/* ── 删除确认弹窗 ── */
const deleteTarget = ref<ServerProfile | null>(null);

function requestDelete(p: ServerProfile) {
  deleteTarget.value = p;
}

function confirmDelete() {
  if (!deleteTarget.value) return;
  deleteProfile(deleteTarget.value.id);
  deleteTarget.value = null;
}

/* ── 连接操作（真实 IPC）── */
async function connect(profileId: string) {
  const p = profiles.value.find((x) => x.id === profileId);
  if (!p) return;
  const existing = connections.value.find((c) => c.profileId === profileId);
  if (existing) {
    activeProfileId.value = profileId;
    return; // 已连接：仅切换
  }
  try {
    // 从后端取加密凭证（密码/私钥），随连接请求发送
    const creds = await ipc.sshCredentialGet(profileId);
    const conn = await ipc.sshConnect({
      profile: p,
      password: creds.password,
      privateKey: creds.privateKey,
      passphrase: creds.passphrase,
    });
    connections.value = connections.value.filter((c) => c.profileId !== profileId);
    connections.value.push(conn);
    activeProfileId.value = profileId;
    p.lastConnectedAt = Date.now();
    persistProfiles(profiles.value);
    ui.toast(`已连接到 ${conn.host ?? p.host}`);
  } catch (e) {
    ui.toast(`连接失败：${e}`);
  }
}

async function disconnect(profileId: string) {
  const conn = connections.value.find((c) => c.profileId === profileId);
  if (!conn?.sessionId) return;
  try {
    await ipc.sshDisconnect(conn.sessionId);
    connections.value = connections.value.filter((c) => c.profileId !== profileId);
    ui.toast("已断开连接");
  } catch (e) {
    ui.toast(`断开失败：${e}`);
  }
}

/* ── 状态事件订阅（后端推送连接/断开/重连）── */
let unlistenConn: (() => void) | null = null;

onMounted(async () => {
  // 初始会话列表 + 状态事件
  try {
    connections.value = await ipc.sshConnections();
    if (connections.value.length > 0 && !activeProfileId.value) {
      activeProfileId.value = connections.value[0].profileId;
    }
  } catch {
    /* 后端未就绪时静默（浏览器预览场景） */
  }
  unlistenConn = await onConnectionStatus((conn) => {
    const idx = connections.value.findIndex((c) => c.profileId === conn.profileId);
    if (idx >= 0) connections.value[idx] = conn;
    else connections.value.push(conn);
  });
});

onUnmounted(() => {
  unlistenConn?.();
});

/* ── 页签工作区 ── */
const tabs = [
  { id: "terminal", name: "终端", component: TerminalTab },
  { id: "files", name: "文件", component: FileManagerTab },
  { id: "monitor", name: "监控", component: MonitorTab },
  { id: "services", name: "服务", component: ServiceTab },
  { id: "processes", name: "进程", component: ProcessTab },
  { id: "docker", name: "Docker", component: DockerTab },
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
      @select="
        (id) => {
          activeProfileId = id;
          connect(id); // 单击即连接（UI.md 要求，UI-016）
        }
      "
      @connect="connect"
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
            :is="activeTab.component"
            :connection="activeConnection"
            :profile="profiles.find((p) => p.id === activeProfileId)"
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
      @error="(msg) => ui.toast(msg)"
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
            确定删除「{{ deleteTarget.name }}」（{{ deleteTarget.host }}）的连接信息？此操作不可撤销。
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
