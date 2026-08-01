<script setup lang="ts">
/**
 * ToolList · 列表视图（对齐原型 .list/.lrow）
 */
import AppIcon from "@/features/ui/AppIcon.vue";
import { useToolsStore } from "@/stores/tools";

const tools = useToolsStore();

const catNames: Record<string, string> = {
  dev: "开发",
  text: "文本",
  image: "图片",
  net: "网络",
  sys: "系统",
};
</script>

<template>
  <div class="overflow-hidden rounded-lg border border-border dark:border-border-dark">
    <div
      v-for="t in tools.filtered"
      :key="t.id"
      class="flex cursor-pointer items-center gap-[13px] border-b border-border bg-surface px-[18px] py-[12px] transition-colors duration-100 last:border-b-0 hover:bg-surface-muted dark:border-border-dark dark:bg-surface-dark dark:hover:bg-surface-muted-dark"
      @click="tools.openTool(t.id)"
    >
      <div
        class="grid h-[34px] w-[34px] shrink-0 place-items-center rounded-[9px] bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
      >
        <AppIcon :name="t.icon" :size="17" />
      </div>
      <div class="min-w-0">
        <div class="text-body font-medium dark:text-primary-dark">{{ t.name }}</div>
        <div class="mt-[1px] truncate text-body-sm text-secondary dark:text-secondary-dark">
          {{ t.description }}
        </div>
      </div>
      <span
        class="ml-auto shrink-0 rounded-full bg-neutral px-[9px] py-[3px] text-caption font-medium text-text-muted dark:bg-neutral-dark dark:text-text-muted-dark"
      >
        {{ catNames[t.category] ?? t.category }}
      </span>
    </div>
  </div>
</template>
