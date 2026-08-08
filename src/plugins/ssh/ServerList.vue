<script setup lang="ts">
/**
 * ServerList · SSH 服务器列表侧栏
 * 列表项仅显示状态点 + 名称（防误触删除）；右键菜单控制连接/断开、编辑、删除；
 * 删除操作由父组件弹确认框（emit deleteRequest）。
 */
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { ServerProfile, ServerConnection } from "./contracts";
import { statusDotClass, statusText } from "./useSsh";

const props = defineProps<{
  profiles: ServerProfile[];
  connections: ServerConnection[];
  activeProfileId: string | null;
  searchKeyword: string;
}>();

const emit = defineEmits<{
  (e: "update:searchKeyword", v: string): void;
  (e: "select", profileId: string): void;
  (e: "connect", profileId: string): void;
  (e: "disconnect", profileId: string): void;
  (e: "add"): void;
  (e: "edit", p: ServerProfile): void;
  (e: "deleteRequest", p: ServerProfile): void;
}>();

function connOf(profileId: string): ServerConnection | undefined {
  return props.connections.find((c) => c.profileId === profileId);
}

/* ── 右键菜单状态 ── */
const menu = ref<{ profile: ServerProfile; x: number; y: number } | null>(null);

/** 右键打开菜单（限制在视口内，避免溢出） */
function openMenu(e: MouseEvent, p: ServerProfile) {
  e.preventDefault();
  const MENU_W = 150;
  const MENU_H = 132;
  const x = Math.min(e.clientX, window.innerWidth - MENU_W - 8);
  const y = Math.min(e.clientY, window.innerHeight - MENU_H - 8);
  menu.value = { profile: p, x, y };
}

function closeMenu() {
  menu.value = null;
}

const menuStatus = computed(() => {
  if (!menu.value) return "disconnected";
  return connOf(menu.value.profile.id)?.status ?? "disconnected";
});

function menuConnectOrDisconnect() {
  if (!menu.value) return;
  const id = menu.value.profile.id;
  if (menuStatus.value === "connected") emit("disconnect", id);
  else emit("connect", id);
  closeMenu();
}

function menuEdit() {
  if (!menu.value) return;
  emit("edit", menu.value.profile);
  closeMenu();
}

function menuDelete() {
  if (!menu.value) return;
  emit("deleteRequest", menu.value.profile);
  closeMenu();
}

onMounted(() => document.addEventListener("mousedown", closeMenu));
onUnmounted(() => document.removeEventListener("mousedown", closeMenu));
</script>

<template>
  <div class="flex w-[180px] shrink-0 flex-col border-r border-border dark:border-border-dark">
    <!-- 搜索 + 添加 -->
    <div class="shrink-0 space-y-[8px] px-[12px] py-[10px]">
      <input
        :value="searchKeyword"
        class="field-input !py-[7px] text-body-sm"
        placeholder="搜索服务器..."
        spellcheck="false"
        @input="emit('update:searchKeyword', ($event.target as HTMLInputElement).value)"
      />
      <button class="btn-secondary w-full text-body-sm" @click="emit('add')">
        + 添加服务器
      </button>
    </div>

    <!-- 服务器列表（仅名称，右键菜单操作） -->
    <div class="min-h-0 flex-1 overflow-y-auto px-[6px] pb-[8px]">
      <div
        v-for="p in profiles"
        :key="p.id"
        class="mb-[2px] flex cursor-pointer items-center gap-[8px] rounded-md px-[8px] py-[8px] transition-colors"
        :class="
          p.id === activeProfileId
            ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark'
            : 'hover:bg-border dark:hover:bg-border-dark'
        "
        :title="`${p.username}@${p.host}:${p.port}（右键操作）`"
        @click="emit('select', p.id)"
        @contextmenu="openMenu($event, p)"
      >
        <span
          class="inline-block h-[8px] w-[8px] shrink-0 rounded-full"
          :class="statusDotClass(connOf(p.id)?.status ?? 'disconnected')"
        />
        <span
          class="min-w-0 flex-1 truncate text-body font-medium text-primary dark:text-primary-dark"
        >
          {{ p.name }}
        </span>
        <span
          class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark"
          :class="{ 'animate-pulse': connOf(p.id)?.status === 'connecting' }"
        >
          {{ statusText(connOf(p.id)?.status ?? "disconnected") }}
        </span>
      </div>

      <p
        v-if="!profiles.length"
        class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        暂无服务器<br />点击「+ 添加服务器」新建配置
      </p>
    </div>

    <!-- 右键菜单（Teleport 到 body，点外部/滚动关闭） -->
    <Teleport to="body">
      <div
        v-if="menu"
        class="fixed z-[200] w-[150px] overflow-hidden rounded-md border border-border bg-surface py-[4px] shadow-[0_8px_24px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark"
        :style="{ left: `${menu.x}px`, top: `${menu.y}px` }"
        @mousedown.stop
      >
        <button
          class="flex w-full items-center px-[12px] py-[7px] text-body text-primary transition-colors hover:bg-surface-muted dark:text-primary-dark dark:hover:bg-surface-muted-dark"
          @click="menuConnectOrDisconnect"
        >
          {{ menuStatus === "connected" ? "断开连接" : "连接" }}
        </button>
        <button
          class="flex w-full items-center px-[12px] py-[7px] text-body text-primary transition-colors hover:bg-surface-muted dark:text-primary-dark dark:hover:bg-surface-muted-dark"
          @click="menuEdit"
        >
          编辑
        </button>
        <div class="my-[4px] border-t border-border dark:border-border-dark" />
        <button
          class="flex w-full items-center px-[12px] py-[7px] text-body text-danger-strong transition-colors hover:bg-danger-soft dark:text-danger-dark dark:hover:bg-danger-soft-dark"
          @click="menuDelete"
        >
          删除
        </button>
      </div>
    </Teleport>
  </div>
</template>
