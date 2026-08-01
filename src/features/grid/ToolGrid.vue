<script setup lang="ts">
/**
 * ToolGrid · 卡片网格（auto-fill minmax(228px,1fr)，对齐 DESIGN.md 布局规范）
 */
import ToolCard from "./ToolCard.vue";
import { useToolsStore } from "@/stores/tools";

const tools = useToolsStore();
</script>

<template>
  <div class="grid grid-cols-[repeat(auto-fill,minmax(228px,1fr))] gap-md">
    <ToolCard v-for="t in tools.filtered" :key="t.id" :tool="t" />
  </div>
</template>
