<script setup lang="ts">
/**
 * RecentStrip · 最近使用快捷条（≤6 个 chips，对齐原型 .recent/.rchip）
 */
import { computed } from "vue";
import AppIcon from "@/features/ui/AppIcon.vue";
import { getTool } from "@/core/registry/toolRegistry";
import { useToolsStore } from "@/stores/tools";

const tools = useToolsStore();

const recentTools = computed(() =>
  tools.recent.map((id) => getTool(id)).filter((t): t is NonNullable<typeof t> => Boolean(t))
);
</script>

<template>
  <div
    v-if="recentTools.length"
    class="mb-lg flex flex-wrap items-center gap-[12px] rounded-lg border border-border bg-gradient-to-br from-tertiary-soft to-surface p-[16px_18px] dark:border-border-dark dark:from-tertiary-soft-dark dark:to-surface-dark"
  >
    <span
      class="mr-[4px] flex items-center gap-[7px] text-body-sm font-semibold text-secondary dark:text-secondary-dark"
    >
      <AppIcon name="ts" :size="15" class="text-tertiary-strong dark:text-tertiary-dark" />
      最近使用
    </span>
    <button
      v-for="t in recentTools"
      :key="t.id"
      class="flex items-center gap-[7px] rounded-full border border-border bg-surface px-[12px] py-[6px] text-body-sm font-medium text-secondary transition-all duration-150 hover:-translate-y-[1px] hover:border-tertiary hover:text-tertiary-strong hover:shadow-[0_1px_2px_rgba(16,24,40,0.04)] dark:border-border-dark dark:bg-surface-dark dark:text-secondary-dark dark:hover:border-tertiary-dark dark:hover:text-tertiary-dark"
      @click="tools.openTool(t.id)"
    >
      <AppIcon :name="t.icon" :size="13" />
      {{ t.name }}
    </button>
  </div>
</template>
