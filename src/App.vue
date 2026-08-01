<script setup lang="ts">
/**
 * App.vue · M0 主界面（方向二空数据版，对齐 sketches/002-clean-light）
 *
 * 布局：侧栏（品牌 + 工具库导航 + 计数徽标 + 主题/设置入口）+ 顶栏（分类标题 + 搜索 + 视图切换 + 添加）
 * M1 起拆分为 features/ 组件树，计数改由工具注册表聚合，搜索接入 fuse.js。
 */
import { ref } from "vue";

interface NavItem {
  id: string;
  label: string;
  icon: string;
}

// 导航：全部工具 + 5 分类 + 我的收藏（原型 002 结构）
const navItems: NavItem[] = [
  { id: "all", label: "全部工具", icon: "all" },
  { id: "dev", label: "开发工具", icon: "dev" },
  { id: "text", label: "文本处理", icon: "text" },
  { id: "image", label: "图片工具", icon: "image" },
  { id: "net", label: "网络工具", icon: "net" },
  { id: "sys", label: "系统工具", icon: "sys" },
  { id: "fav", label: "我的收藏", icon: "fav" },
];

// 顶栏标题映射（原型 names 表）
const titleMap: Record<string, string> = {
  all: "全部工具",
  dev: "开发工具",
  text: "文本处理",
  image: "图片工具",
  net: "网络工具",
  sys: "系统工具",
  fav: "我的收藏",
};

// M1 起由工具注册表统计；空数据版全部为 0
const toolCounts: Record<string, number> = {
  all: 0,
  dev: 0,
  text: 0,
  image: 0,
  net: 0,
  sys: 0,
  fav: 0,
};

// 图标表（对齐原型 002 线性图标集；M1 收敛为全局图标表组件）
const iconInner: Record<string, string> = {
  all: '<rect x="3.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.8"/>',
  dev: '<path d="M8.5 6 4 12l4.5 6M15.5 6 20 12l-4.5 6"/>',
  text: '<path d="M5 7h14M5 12h9M5 17h14"/>',
  image:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="M4 17l5-4 3.5 3 3.5-3.5L20 17"/>',
  net: '<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.4 2.4 3.8 5.3 3.8 8.5s-1.4 6.1-3.8 8.5c-2.4-2.4-3.8-5.3-3.8-8.5s1.4-6.1 3.8-8.5z"/>',
  sys: '<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2.2"/><circle cx="8" cy="17" r="2.2"/>',
  fav: '<path d="M12 3.5l2.6 5.5 6 .7-4.4 4.2 1.2 6L12 17l-5.4 2.9 1.2-6L3.4 9.7l6-.7z"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="M16.5 16.5 21 21"/>',
  grid: '<rect x="3.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.8"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.8"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.8"/>',
  list: '<path d="M9 6h11M9 12h11M9 18h11M4 6h.01M4 12h.01M4 18h.01"/>',
  add: '<path d="M12 5v14M5 12h14"/>',
  moon: '<path d="M12 3.5a8.5 8.5 0 1 0 8.5 8.5c-4.5 0-8.5-4-8.5-8.5z"/>',
  sun: '<circle cx="12" cy="12" r="4.5"/><path d="M12 2.8v2M12 19.2v2M2.8 12h2M19.2 12h2M5.4 5.4l1.4 1.4M17.2 17.2l1.4 1.4M18.6 5.4l-1.4 1.4M6.8 17.2l-1.4 1.4"/>',
  gear: '<circle cx="12" cy="12" r="3.2"/><path d="M12 2.8v2.3M12 18.9v2.3M2.8 12h2.3M18.9 12h2.3M5.4 5.4l1.6 1.6M17 17l1.6 1.6M18.6 5.4 17 7M7 17l-1.6 1.6"/>',
};

const activeCategory = ref("all");
const listView = ref(false);
const isDark = ref(false);

/** 深色模式基础版：切 html[data-theme=dark]；持久化与跟随系统由 M1 设置模块接管 */
function toggleTheme() {
  isDark.value = !isDark.value;
  document.documentElement.dataset.theme = isDark.value ? "dark" : "";
}
</script>

<template>
  <div class="flex h-screen overflow-hidden">
    <!-- 侧栏 -->
    <aside
      class="flex w-[236px] shrink-0 flex-col border-r border-border bg-surface-muted px-md pb-[16px] pt-lg dark:border-border-dark dark:bg-surface-muted-dark"
    >
      <div class="mb-lg flex items-center gap-[10px] px-[10px]">
        <div
          class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-gradient-to-br from-tertiary to-tertiary-strong text-[17px] font-extrabold text-on-tertiary shadow-[0_4px_12px_rgba(240,86,44,0.35)]"
        >
          P
        </div>
        <div>
          <div class="text-[15px] font-bold tracking-[-0.01em] dark:text-primary-dark">
            patchyBox
          </div>
          <div
            class="mt-[1px] text-[10.5px] font-medium tracking-[0.06em] text-text-muted dark:text-text-muted-dark"
          >
            DESKTOP TOOLBOX
          </div>
        </div>
      </div>

      <div
        class="mb-[6px] mt-md px-[10px] text-[10.5px] font-semibold tracking-[0.1em] text-text-muted dark:text-text-muted-dark"
      >
        工具库
      </div>
      <nav class="flex flex-1 flex-col gap-[2px] overflow-y-auto">
        <button
          v-for="item in navItems"
          :key="item.id"
          class="flex w-full items-center gap-[10px] rounded-sm px-[10px] py-[9px] text-[13.5px] font-medium transition-colors duration-150 dark:transition-none"
          :class="
            activeCategory === item.id
              ? 'bg-tertiary-soft font-semibold text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
              : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
          "
          @click="activeCategory = item.id"
        >
          <svg
            class="h-[17px] w-[17px] shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            v-html="iconInner[item.icon]"
          />
          <span class="flex-1 text-left">{{ item.label }}</span>
          <span
            class="rounded-full px-[7px] py-[1px] text-[11px] font-semibold"
            :class="
              activeCategory === item.id
                ? 'bg-tertiary-strong text-on-tertiary dark:bg-tertiary-dark dark:text-on-tertiary-dark'
                : 'bg-border text-text-muted dark:bg-border-dark dark:text-text-muted-dark'
            "
          >
            {{ toolCounts[item.id] }}
          </span>
        </button>
      </nav>

      <div
        class="mt-md flex flex-col gap-[2px] border-t border-border pt-md dark:border-border-dark"
      >
        <button
          class="flex items-center gap-[10px] rounded-sm px-[10px] py-[9px] text-[13px] font-medium text-secondary transition-colors duration-150 hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          @click="toggleTheme"
        >
          <svg
            class="h-4 w-4 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            v-html="isDark ? iconInner.sun : iconInner.moon"
          />
          {{ isDark ? "浅色模式" : "深色模式" }}
        </button>
        <button
          class="flex items-center gap-[10px] rounded-sm px-[10px] py-[9px] text-[13px] font-medium text-secondary transition-colors duration-150 hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          title="偏好设置（M1 开放）"
        >
          <svg
            class="h-4 w-4 shrink-0"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            v-html="iconInner.gear"
          />
          偏好设置
        </button>
      </div>
    </aside>

    <!-- 主区域 -->
    <main class="flex min-w-0 flex-1 flex-col">
      <header
        class="flex h-[66px] shrink-0 items-center gap-md border-b border-border bg-surface px-xl dark:border-border-dark dark:bg-surface-dark"
      >
        <div>
          <h1 class="text-[17px] font-bold tracking-[-0.02em] dark:text-primary-dark">
            {{ titleMap[activeCategory] }}
          </h1>
          <p class="mt-[1px] text-[12px] text-text-muted dark:text-text-muted-dark">
            共 {{ toolCounts[activeCategory] }} 个工具 · 点击卡片即可使用
          </p>
        </div>
        <div class="flex-1" />
        <div
          class="flex h-[38px] w-[280px] items-center gap-sm rounded-md border border-border bg-neutral px-[12px] transition-all duration-200 focus-within:w-[320px] focus-within:border-tertiary focus-within:shadow-[0_0_0_3px_var(--color-tertiary-soft)] dark:border-border-dark dark:bg-neutral-dark"
        >
          <svg
            class="h-[15px] w-[15px] shrink-0 text-text-muted dark:text-text-muted-dark"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            v-html="iconInner.search"
          />
          <input
            class="flex-1 bg-transparent text-[13px] text-primary outline-none placeholder:text-text-muted dark:text-primary-dark dark:placeholder:text-text-muted-dark"
            type="text"
            placeholder="搜索工具…"
            spellcheck="false"
          />
        </div>
        <button
          class="grid h-[38px] w-[38px] shrink-0 place-items-center rounded-md border border-border bg-surface text-secondary transition-colors duration-150 hover:border-border-strong hover:bg-surface-muted hover:text-primary dark:border-border-dark dark:bg-surface-dark dark:text-secondary-dark dark:hover:border-border-strong-dark dark:hover:bg-surface-muted-dark dark:hover:text-primary-dark"
          title="视图切换"
          @click="listView = !listView"
        >
          <svg
            class="h-[17px] w-[17px]"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            v-html="listView ? iconInner.list : iconInner.grid"
          />
        </button>
        <button
          class="grid h-[38px] w-[38px] shrink-0 place-items-center rounded-md bg-tertiary text-on-tertiary transition-[filter] duration-150 hover:brightness-110 dark:bg-tertiary-dark dark:text-on-tertiary-dark"
          title="添加工具（M1 开放）"
        >
          <svg
            class="h-[17px] w-[17px]"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
            stroke-linecap="round"
            v-html="iconInner.add"
          />
        </button>
      </header>

      <div class="flex-1 overflow-y-auto px-xl py-lg">
        <div class="mb-[12px] flex items-center">
          <h2 class="text-[14px] font-bold tracking-[-0.01em] dark:text-primary-dark">工具列表</h2>
        </div>
        <!-- 空状态（M1 替换为工具卡片网格） -->
        <div class="flex h-[calc(100%-28px)] flex-col items-center justify-center text-center">
          <div
            class="grid h-11 w-11 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
          >
            <svg
              class="h-[22px] w-[22px] text-tertiary-strong dark:text-tertiary-dark"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              v-html="iconInner.all"
            />
          </div>
          <p class="mt-md text-[14px] font-bold dark:text-primary-dark">暂无工具</p>
          <p class="mt-xs text-[12px] text-text-muted dark:text-text-muted-dark">
            首批 8 个文本工具将在 M1 上线
          </p>
        </div>
      </div>
    </main>
  </div>
</template>
