<script setup lang="ts">
/**
 * HttpSidebar · 左侧请求历史栏（Postman 式：方法色标 + URL + 相对时间，点击回填）
 */
import type { HistoryRecord } from "@/core/ipc/contracts";
import { formatRelativeTime } from "./useHttp";

defineProps<{
  records: HistoryRecord[];
  activeId: number | null;
}>();

const emit = defineEmits<{
  (e: "select", r: HistoryRecord): void;
  (e: "clear"): void;
}>();

function methodClass(m: string): string {
  if (m === "GET")
    return "bg-success-soft text-success-strong dark:bg-success-soft-dark dark:text-success-dark";
  if (["POST", "PUT", "PATCH"].includes(m))
    return "bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark";
  return "bg-neutral text-secondary dark:bg-neutral-dark dark:text-secondary-dark";
}
</script>

<template>
  <div class="flex w-[240px] shrink-0 flex-col border-r border-border dark:border-border-dark">
    <div class="flex shrink-0 items-center justify-between px-[12px] py-[10px]">
      <span class="text-caption font-medium text-text-muted dark:text-text-muted-dark">
        请求历史（{{ records.length }}）
      </span>
      <button
        class="btn-ghost !px-[6px] !py-[2px] text-body-sm"
        title="清空历史"
        @click="emit('clear')"
      >
        清空
      </button>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-[6px] pb-[8px]">
      <div
        v-for="r in records"
        :key="r.id"
        class="mb-[2px] flex cursor-pointer flex-col gap-[3px] rounded-md px-[8px] py-[7px] transition-colors"
        :class="
          r.id === activeId
            ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark'
            : 'hover:bg-border dark:hover:bg-border-dark'
        "
        :title="`${r.method} ${r.url}\n${r.createdAt}`"
        @click="emit('select', r)"
      >
        <div class="flex items-center gap-[8px]">
          <span
            class="w-[46px] shrink-0 rounded-[4px] px-[4px] py-[1px] text-center font-mono text-caption font-medium"
            :class="methodClass(r.method)"
            >{{ r.method }}</span
          >
          <span
            v-if="r.status"
            class="shrink-0 font-mono text-body-sm"
            :class="
              r.status >= 400
                ? 'text-tertiary-strong dark:text-tertiary-dark'
                : 'text-success-strong dark:text-success-dark'
            "
          >
            {{ r.status }}
          </span>
          <span class="ml-auto shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
            {{ formatRelativeTime(r.createdAt) }}
          </span>
        </div>
        <span class="truncate font-mono text-body-sm text-secondary dark:text-secondary-dark">
          {{ r.url }}
        </span>
      </div>

      <p
        v-if="!records.length"
        class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        暂无历史<br />发送请求后自动保存
      </p>
    </div>
  </div>
</template>
