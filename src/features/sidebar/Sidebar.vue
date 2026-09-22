<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * Sidebar · 侧栏（全局模糊搜索 + 工具库导航 + 计数徽标 + 主题/设置入口）
 * 导航项与计数来自工具注册表聚合（tools store）；主题切换走 settings store；
 * 搜索下拉选中（点击/Enter）直接打开工具页签。
 */
import { computed, ref } from 'vue'
import AppIcon from '@/features/ui/AppIcon.vue'
import ResourceMonitor from './ResourceMonitor.vue'
import { useI18n } from 'vue-i18n'
import type { ToolManifest } from '@/core/registry/types'
import { searchTools, type HighlightChunk } from '@/core/search/fuzzy'
import { useSettingsStore } from '@/stores/settings'
import { useToolsStore } from '@/stores/tools'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
const tools = useToolsStore()
const settings = useSettingsStore()
const { t } = useI18n()

const searchInput = ref<HTMLInputElement | null>(null)
const searchFocused = ref(false)
const activeIndex = ref(-1)

/** 模糊匹配结果（全局搜索，不随当前分类过滤；带名称高亮分片） */
const matches = computed<{ tool: ToolManifest; chunks: HighlightChunk[] | null }[]>(() => {
  const q = ui.searchQuery.trim()
  if (!q) return []
  return searchTools(q, 8).map((tool) => ({ tool, chunks: tools.nameChunks(tool) }))
})

const navItems = [
  { id: 'all', labelKey: 'nav.all', icon: 'all' },
  { id: 'dev', labelKey: 'nav.dev', icon: 'dev' },
  { id: 'text', labelKey: 'nav.text', icon: 'text' },
  { id: 'image', labelKey: 'nav.image', icon: 'image' },
  { id: 'net', labelKey: 'nav.net', icon: 'net' },
  { id: 'sys', labelKey: 'nav.sys', icon: 'sys' },
  { id: 'fav', labelKey: 'nav.fav', icon: 'fav' },
]

function toggleTheme() {
  settings.set('theme', settings.settings.theme === 'dark' ? 'light' : 'dark')
}
/** 聚焦搜索时若在工具页签，切回工具库首页 */
function onSearchFocus() {
  searchFocused.value = true
  if (ui.activeTab) ui.goHome()
}
/** 下拉键盘导航：↑↓ 选择、Enter 打开（未选中时打开第一条）、Esc 关闭 */
function onSearchKeydown(event: KeyboardEvent) {
  if (!matches.value.length) return
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    activeIndex.value = Math.min(activeIndex.value + 1, matches.value.length - 1)
  } else if (event.key === 'ArrowUp') {
    event.preventDefault()
    activeIndex.value = Math.max(activeIndex.value - 1, -1)
  } else if (event.key === 'Enter') {
    const match = matches.value[activeIndex.value >= 0 ? activeIndex.value : 0]
    if (match) selectTool(match.tool)
  } else if (event.key === 'Escape') {
    searchInput.value?.blur()
  }
}
/** 选中下拉项：直接打开工具并复位搜索框 */
function selectTool(tool: ToolManifest) {
  tools.openTool(tool.id)
  ui.searchQuery = ''
  activeIndex.value = -1
  searchFocused.value = false
  searchInput.value?.blur()
}
/** 分类导航：切换分类并回到工具库首页（退出设置页） */
function selectCategory(item: { id: string }) {
  ui.activeCategory = item.id
  ui.searchQuery = ''
  ui.goHome()
  ui.closeSettings()
}
</script>

<template>
  <aside
    class="flex w-[200px] shrink-0 flex-col border-r border-border bg-surface-muted px-md pb-[16px] pt-lg dark:border-border-dark dark:bg-surface-muted-dark"
  >
    <!-- 全局模糊搜索（品牌标识由标题栏承载，侧栏不再重复；下拉选中直接打开工具） -->
    <div class="relative mb-md px-[10px]">
      <div
        class="flex h-[36px] w-full items-center gap-sm rounded-md border border-border bg-neutral px-[12px] transition-colors duration-150 focus-within:border-border-strong dark:border-border-dark dark:bg-neutral-dark dark:focus-within:border-border-strong-dark"
      >
        <AppIcon
          name="search"
          :size="15"
          class="shrink-0 text-text-muted dark:text-text-muted-dark"
        />
        <input
          ref="searchInput"
          v-model="ui.searchQuery"
          autocomplete="off"
          class="min-w-0 flex-1 bg-transparent text-body text-primary outline-none placeholder:text-text-muted dark:text-primary-dark dark:placeholder:text-text-muted-dark"
          type="text"
          :placeholder="t('topbar.search')"
          spellcheck="false"
          @focus="onSearchFocus"
          @blur="searchFocused = false"
          @keydown="onSearchKeydown"
        />
        <button
          v-if="ui.searchQuery"
          class="grid h-[18px] w-[18px] shrink-0 place-items-center rounded-full text-caption leading-none text-text-muted transition-colors duration-100 hover:bg-border hover:text-primary dark:hover:bg-border-dark dark:hover:text-primary-dark"
          aria-label="清空搜索"
          @click="ui.searchQuery = ''"
        >
          ×
        </button>
      </div>
      <!-- 搜索下拉：↑↓ 选择、Enter / 点击直接打开工具 -->
      <UiScrollArea v-if="searchFocused && matches.length" as-child axis="vertical">
        <div
          class="absolute inset-x-[10px] top-full z-30 mt-[6px] max-h-[320px] rounded-md border border-border bg-surface py-[4px] shadow-[0_12px_40px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark"
        >
          <button
            v-for="(match, i) in matches"
            :key="match.tool.id"
            class="flex w-full items-center gap-[9px] px-[10px] py-[8px] text-left transition-colors duration-100"
            :class="i === activeIndex ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark' : ''"
            @mousedown.prevent
            @click="selectTool(match.tool)"
            @mousemove="activeIndex = i"
          >
            <AppIcon
              :name="match.tool.icon"
              :size="16"
              class="shrink-0 text-tertiary-strong dark:text-tertiary-dark"
            />
            <span
              class="min-w-0 flex-1 truncate text-body font-medium text-primary dark:text-primary-dark"
            >
              <template v-if="match.chunks">
                <template v-for="(c, ci) in match.chunks" :key="ci">
                  <mark
                    v-if="c.hit"
                    class="rounded-[2px] bg-tertiary-soft px-[1px] text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
                    >{{ c.text }}</mark
                  >
                  <template v-else>{{ c.text }}</template>
                </template>
              </template>
              <template v-else>{{ match.tool.name }}</template>
            </span>
          </button>
        </div>
      </UiScrollArea>
    </div>

    <!-- 导航 -->
    <div
      class="mb-[6px] mt-md px-[10px] text-label-caps font-semibold tracking-[0.1em] text-text-muted dark:text-text-muted-dark"
    >
      {{ t('nav.library') }}
    </div>
    <UiScrollArea as-child axis="vertical">
      <nav class="flex flex-1 flex-col gap-[2px]">
        <button
          v-for="item in navItems"
          :key="item.id"
          class="flex w-full items-center gap-[10px] rounded-sm px-[10px] py-[9px] text-body font-medium transition-colors duration-150"
          :class="
            ui.activeCategory === item.id
              ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
              : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
          "
          @click="selectCategory(item)"
        >
          <AppIcon :name="item.icon" :size="17" class="shrink-0" />
          <span class="flex-1 text-left">{{ t(item.labelKey) }}</span>
          <span
            class="rounded-full px-[7px] py-[1px] text-caption font-semibold"
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
    </UiScrollArea>

    <!-- 底部固定区：资源统计独立成栏，主题与设置保持同一菜单组。 -->
    <div
      class="mt-md shrink-0 pt-md"
      :class="
        !settings.settings.resourceMonitorEnabled
          ? 'border-t border-border dark:border-border-dark'
          : ''
      "
    >
      <section
        v-if="settings.settings.resourceMonitorEnabled"
        aria-label="应用资源统计"
        class="mb-sm border-b border-border pb-sm dark:border-border-dark"
      >
        <ResourceMonitor />
      </section>
      <div class="flex flex-col gap-[2px]">
        <button
          class="flex items-center gap-[10px] rounded-sm px-[10px] py-[9px] text-body font-medium text-secondary transition-colors duration-150 hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          @click="toggleTheme"
        >
          <AppIcon
            :name="settings.settings.theme === 'dark' ? 'sun' : 'moon'"
            :size="16"
            class="shrink-0"
          />
          {{ settings.settings.theme === 'dark' ? t('nav.lightMode') : t('nav.darkMode') }}
        </button>
        <button
          class="flex items-center gap-[10px] rounded-sm px-[10px] py-[9px] text-body font-medium transition-colors duration-150 hover:bg-border hover:text-primary dark:hover:bg-border-dark dark:hover:text-primary-dark"
          :class="
            ui.settingsOpen
              ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
              : 'text-secondary dark:text-secondary-dark'
          "
          @click="ui.toggleSettings()"
        >
          <AppIcon name="sliders" :size="16" class="shrink-0" />
          {{ t('common.settings') }}
        </button>
      </div>
    </div>
  </aside>
</template>
