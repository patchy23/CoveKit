<script setup lang="ts">
/**
 * ServerList · SSH 服务器列表侧栏
 * 搜索 + 状态点 + 操作按钮（连接/断开/编辑/删除）
 */
import type { ServerProfile, ServerConnection } from "./contracts";
import { statusDotClass, statusText, formatLatency } from "./useSsh";

defineProps<{
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
  (e: "delete", id: string): void;
}>();

function connOf(profileId: string, connections: ServerConnection[]) {
  return connections.find((c) => c.profileId === profileId);
}
</script>

<template>
  <div class="flex w-[240px] shrink-0 flex-col border-r border-border dark:border-border-dark">
    <!-- 搜索 + 添加 -->
    <div class="shrink-0 space-y-[8px] px-[12px] py-[10px]">
      <input
        :value="searchKeyword"
        class="field-input !py-[7px] text-body-sm"
        placeholder="搜索服务器..."
        spellcheck="false"
        @input="emit('update:searchKeyword', ($event.target as HTMLInputElement).value)"
      />
      <button class="btn-secondary w-full !h-[32px] text-body-sm" @click="emit('add')">
        + 添加服务器
      </button>
    </div>

    <!-- 服务器列表 -->
    <div class="min-h-0 flex-1 overflow-y-auto px-[6px] pb-[8px]">
      <div
        v-for="p in profiles"
        :key="p.id"
        class="group mb-[2px] flex cursor-pointer flex-col gap-[4px] rounded-md px-[8px] py-[8px] transition-colors"
        :class="
          p.id === activeProfileId
            ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark'
            : 'hover:bg-border dark:hover:bg-border-dark'
        "
        @click="emit('select', p.id)"
      >
        <div class="flex items-center gap-[8px]">
          <span
            class="inline-block h-[8px] w-[8px] shrink-0 rounded-full"
            :class="statusDotClass(connOf(p.id, connections)?.status ?? 'disconnected')"
          />
          <span
            class="min-w-0 flex-1 truncate text-body font-medium text-primary dark:text-primary-dark"
          >
            {{ p.name }}
          </span>
          <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
            {{ statusText(connOf(p.id, connections)?.status ?? "disconnected") }}
          </span>
        </div>
        <div class="flex items-center gap-[6px] pl-[16px]">
          <span class="truncate font-mono text-body-sm text-secondary dark:text-secondary-dark">
            {{ p.username }}@{{ p.host }}:{{ p.port }}
          </span>
          <span
            v-if="connOf(p.id, connections)?.latencyMs"
            class="ml-auto shrink-0 text-caption text-text-muted dark:text-text-muted-dark"
          >
            {{ formatLatency(connOf(p.id, connections)?.latencyMs) }}
          </span>
        </div>
        <!-- hover 操作按钮 -->
        <div
          class="hidden items-center gap-[4px] pl-[16px] pt-[2px] group-hover:flex"
          @click.stop
        >
          <button
            v-if="connOf(p.id, connections)?.status !== 'connected'"
            class="btn-ghost !px-[6px] !py-[2px] text-caption"
            @click="emit('connect', p.id)"
          >
            连接
          </button>
          <button
            v-else
            class="btn-ghost !px-[6px] !py-[2px] text-caption"
            @click="emit('disconnect', p.id)"
          >
            断开
          </button>
          <button
            class="btn-ghost !px-[6px] !py-[2px] text-caption"
            @click="emit('edit', p)"
          >
            编辑
          </button>
          <button
            class="btn-ghost !px-[6px] !py-[2px] text-caption text-danger-strong hover:!text-danger-strong dark:text-danger-dark"
            @click="emit('delete', p.id)"
          >
            删除
          </button>
        </div>
      </div>

      <p
        v-if="!profiles.length"
        class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        暂无服务器<br />点击「+ 添加服务器」新建配置
      </p>
    </div>
  </div>
</template>
