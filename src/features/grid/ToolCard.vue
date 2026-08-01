<script setup lang="ts">
/**
 * ToolCard · 工具卡片（图标块 + 标题[搜索高亮] + 描述 + 标签 + 收藏星）
 * hover 上浮 + 顶部 3px 渐变线（对齐原型 .card 与 DESIGN.md card 规范）。
 */
import { computed } from "vue";
import AppIcon from "@/features/ui/AppIcon.vue";
import type { ToolManifest } from "@/core/registry/types";
import { useFavoritesStore } from "@/stores/favorites";
import { useToolsStore } from "@/stores/tools";
import { useUiStore } from "@/stores/ui";

const props = defineProps<{ tool: ToolManifest }>();

const tools = useToolsStore();
const favorites = useFavoritesStore();
const ui = useUiStore();

const nameChunks = computed(() => tools.nameChunks(props.tool));
const isFav = computed(() => favorites.has(props.tool.id));

async function toggleFav() {
  const nowFav = await favorites.toggle(props.tool.id);
  ui.toast(nowFav ? `已收藏「${props.tool.name}」` : `已取消收藏「${props.tool.name}」`);
}
</script>

<template>
  <div
    class="group relative cursor-pointer overflow-hidden rounded-lg border border-border bg-surface p-[18px] transition-all duration-150 hover:-translate-y-[3px] hover:border-border-strong hover:shadow-[0_12px_40px_rgba(16,24,40,0.14)] dark:border-border-dark dark:bg-surface-dark dark:hover:border-border-strong-dark"
    @click="tools.openTool(tool.id)"
  >
    <!-- hover 顶部渐变线（可点击信号） -->
    <div
      class="pointer-events-none absolute inset-x-0 top-0 h-[3px] bg-gradient-to-r from-tertiary to-transparent opacity-0 transition-opacity duration-150 group-hover:opacity-100"
    />
    <div class="flex items-start justify-between">
      <div
        class="grid h-11 w-11 shrink-0 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
      >
        <AppIcon
          :name="tool.icon"
          :size="22"
          class="text-tertiary-strong dark:text-tertiary-dark"
        />
      </div>
      <button
        class="rounded-[6px] p-1 text-h1 leading-none transition-all duration-150 hover:scale-110"
        :class="isFav ? 'text-tertiary-strong' : 'text-text-muted hover:text-tertiary-strong'"
        :title="isFav ? '取消收藏' : '收藏'"
        @click.stop="toggleFav"
      >
        {{ isFav ? "★" : "☆" }}
      </button>
    </div>
    <h3 class="mt-md text-card-title font-bold tracking-[-0.01em] dark:text-primary-dark">
      <template v-if="nameChunks">
        <template v-for="(c, i) in nameChunks" :key="i">
          <mark
            v-if="c.hit"
            class="rounded-[2px] bg-tertiary-soft px-[1px] text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
            >{{ c.text }}</mark
          >
          <template v-else>{{ c.text }}</template>
        </template>
      </template>
      <template v-else>{{ tool.name }}</template>
    </h3>
    <p class="mt-[5px] text-body-sm leading-[1.55] text-secondary dark:text-secondary-dark">
      {{ tool.description }}
    </p>
    <div v-if="tool.tags?.length" class="mt-[13px] flex gap-[6px]">
      <span
        v-for="tag in tool.tags"
        :key="tag"
        class="rounded-[6px] px-2 py-[3px] text-label-caps font-semibold"
        :class="
          tag === '热门'
            ? 'bg-success-soft text-success-strong dark:bg-success-soft-dark dark:text-success-dark'
            : 'bg-neutral text-text-muted dark:bg-neutral-dark dark:text-text-muted-dark'
        "
      >
        {{ tag }}
      </span>
    </div>
  </div>
</template>
