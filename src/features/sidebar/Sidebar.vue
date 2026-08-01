<script setup lang="ts">
/**
 * Sidebar · 侧栏（品牌 + 工具库导航 + 计数徽标 + 主题/设置入口）
 * 导航项与计数来自工具注册表聚合（tools store）；主题切换走 settings store。
 */
import AppIcon from "@/features/ui/AppIcon.vue";
import { useSettingsStore } from "@/stores/settings";
import { useToolsStore } from "@/stores/tools";
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();
const tools = useToolsStore();
const settings = useSettingsStore();

const navItems = [
  { id: "all", label: "全部工具", icon: "all" },
  { id: "dev", label: "开发工具", icon: "dev" },
  { id: "text", label: "文本处理", icon: "text" },
  { id: "image", label: "图片工具", icon: "image" },
  { id: "net", label: "网络工具", icon: "net" },
  { id: "sys", label: "系统工具", icon: "sys" },
  { id: "fav", label: "我的收藏", icon: "fav" },
];

function toggleTheme() {
  settings.set("theme", settings.settings.theme === "dark" ? "light" : "dark");
}
</script>

<template>
  <aside
    class="flex w-[236px] shrink-0 flex-col border-r border-border bg-surface-muted px-md pb-[16px] pt-lg dark:border-border-dark dark:bg-surface-muted-dark"
  >
    <!-- 品牌 -->
    <div class="mb-lg flex items-center gap-[10px] px-[10px]">
      <div
        class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-gradient-to-br from-tertiary to-tertiary-strong text-[17px] font-extrabold text-on-tertiary shadow-[0_4px_12px_rgba(240,86,44,0.35)]"
      >
        P
      </div>
      <div>
        <div class="text-[15px] font-bold tracking-[-0.01em] dark:text-primary-dark">patchyBox</div>
        <div
          class="mt-[1px] text-[10.5px] font-medium tracking-[0.06em] text-text-muted dark:text-text-muted-dark"
        >
          DESKTOP TOOLBOX
        </div>
      </div>
    </div>

    <!-- 导航 -->
    <div
      class="mb-[6px] mt-md px-[10px] text-[10.5px] font-semibold tracking-[0.1em] text-text-muted dark:text-text-muted-dark"
    >
      工具库
    </div>
    <nav class="flex flex-1 flex-col gap-[2px] overflow-y-auto">
      <button
        v-for="item in navItems"
        :key="item.id"
        class="flex w-full items-center gap-[10px] rounded-sm px-[10px] py-[9px] text-[13.5px] font-medium transition-colors duration-150"
        :class="
          ui.activeCategory === item.id
            ? 'bg-tertiary-soft font-semibold text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
            : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
        "
        @click="
          ui.activeCategory = item.id;
          ui.searchQuery = '';
        "
      >
        <AppIcon :name="item.icon" :size="17" class="shrink-0" />
        <span class="flex-1 text-left">{{ item.label }}</span>
        <span
          class="rounded-full px-[7px] py-[1px] text-[11px] font-semibold"
          :class="
            ui.activeCategory === item.id
              ? 'bg-tertiary-strong text-on-tertiary dark:bg-tertiary-dark dark:text-on-tertiary-dark'
              : 'bg-border text-text-muted dark:bg-border-dark dark:text-text-muted-dark'
          "
        >
          {{ tools.categoryCounts[item.id] ?? 0 }}
        </span>
      </button>
    </nav>

    <!-- 底部入口 -->
    <div class="mt-md flex flex-col gap-[2px] border-t border-border pt-md dark:border-border-dark">
      <button
        class="flex items-center gap-[10px] rounded-sm px-[10px] py-[9px] text-[13px] font-medium text-secondary transition-colors duration-150 hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        @click="toggleTheme"
      >
        <AppIcon
          :name="settings.settings.theme === 'dark' ? 'sun' : 'moon'"
          :size="16"
          class="shrink-0"
        />
        {{ settings.settings.theme === "dark" ? "浅色模式" : "深色模式" }}
      </button>
      <button
        class="flex items-center gap-[10px] rounded-sm px-[10px] py-[9px] text-[13px] font-medium text-secondary transition-colors duration-150 hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        @click="ui.settingsVisible = true"
      >
        <AppIcon name="gear" :size="16" class="shrink-0" />
        偏好设置
      </button>
    </div>
  </aside>
</template>
