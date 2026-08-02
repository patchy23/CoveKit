<script setup lang="ts">
/**
 * ApiSidebar · 接口列表侧栏（Postman Collections 式：保存的接口随时切换）
 */
import type { ApiRecord } from "@/core/ipc/contracts";
import { formatRelativeTime, methodBadgeClass } from "./useHttp";

defineProps<{
  apis: ApiRecord[];
  activeId: number | null;
}>();

const emit = defineEmits<{
  (e: "select", r: ApiRecord): void;
  (e: "rename", r: ApiRecord): void;
  (e: "delete", id: number): void;
  (e: "clear"): void;
  (e: "new"): void;
}>();
</script>

<template>
  <div class="flex w-[240px] shrink-0 flex-col border-r border-border dark:border-border-dark">
    <div class="flex shrink-0 items-center justify-between px-[12px] py-[10px]">
      <span class="text-caption font-medium text-text-muted dark:text-text-muted-dark">
        接口列表（{{ apis.length }}）
      </span>
      <div class="flex items-center gap-[4px]">
        <button
          class="btn-ghost !px-[6px] !py-[2px] text-body-sm"
          title="清空全部接口"
          @click="emit('clear')"
        >
          清空
        </button>
        <button
          class="btn-secondary !px-[8px] !py-[2px] text-body-sm"
          title="新建接口（清空当前表单）"
          @click="emit('new')"
        >
          + 新建
        </button>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-[6px] pb-[8px]">
      <div
        v-for="a in apis"
        :key="a.id"
        class="group mb-[2px] flex cursor-pointer flex-col gap-[3px] rounded-md px-[8px] py-[7px] transition-colors"
        :class="
          a.id === activeId
            ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark'
            : 'hover:bg-border dark:hover:bg-border-dark'
        "
        :title="`${a.method} ${a.url}\n${a.updatedAt}`"
        @click="emit('select', a)"
      >
        <div class="flex items-center gap-[8px]">
          <span
            class="w-[46px] shrink-0 rounded-[4px] px-[4px] py-[1px] text-center font-mono text-caption font-medium"
            :class="methodBadgeClass(a.method, a.type)"
            >{{ a.type === "ws" ? "WS" : a.method }}</span
          >
          <span
            class="min-w-0 flex-1 truncate text-body font-medium text-primary dark:text-primary-dark"
          >
            {{ a.name || "(未命名)" }}
          </span>
          <button
            class="hidden shrink-0 text-text-muted transition-colors hover:text-info-strong group-hover:block dark:text-text-muted-dark dark:hover:text-info-dark"
            title="重命名接口"
            @click.stop="emit('rename', a)"
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M17 3a2.8 2.8 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
            </svg>
          </button>
          <button
            class="hidden shrink-0 text-text-muted transition-colors hover:text-tertiary-strong group-hover:block dark:text-text-muted-dark dark:hover:text-tertiary-dark"
            title="删除接口"
            @click.stop="emit('delete', a.id)"
          >
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
            >
              <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />
            </svg>
          </button>
        </div>
        <div class="flex items-center gap-[6px]">
          <span class="truncate font-mono text-body-sm text-secondary dark:text-secondary-dark">
            {{ a.url }}
          </span>
          <span class="ml-auto shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
            {{ formatRelativeTime(a.updatedAt) }}
          </span>
        </div>
      </div>

      <p
        v-if="!apis.length"
        class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        暂无接口<br />填写请求后点「保存」加入列表
      </p>
    </div>
  </div>
</template>
