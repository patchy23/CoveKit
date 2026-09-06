<script setup lang="ts">
/**
 * TopBar · 顶栏
 * 首页：分类标题 + 计数副标题；工具页签激活：显示工具名 + 描述。
 * 搜索框迁至侧栏顶部、视图切换迁至「工具列表」行（2026-09-06）。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { getTool } from '@/core/registry/toolRegistry'
import { useToolsStore } from '@/stores/tools'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
const tools = useToolsStore()
const { t } = useI18n()

const titleKeys: Record<string, string> = {
  all: 'nav.all',
  dev: 'nav.dev',
  text: 'nav.text',
  image: 'nav.image',
  net: 'nav.net',
  sys: 'nav.sys',
  fav: 'nav.fav',
}

const activeTool = computed(() => (ui.activeTab ? getTool(ui.activeTab) : undefined))
const title = computed(() => activeTool.value?.name ?? t(titleKeys[ui.activeCategory] ?? 'nav.all'))
const subtitle = computed(() =>
  activeTool.value
    ? activeTool.value.description
    : t('topbar.summary', { count: tools.filtered.length })
)
</script>

<template>
  <header
    class="flex h-[48px] shrink-0 items-center gap-md border-b border-border bg-surface px-xl dark:border-border-dark dark:bg-surface-dark"
  >
    <div class="flex min-w-0 items-baseline gap-[10px]">
      <h1 class="shrink-0 truncate text-h1 font-bold tracking-[-0.02em] dark:text-primary-dark">
        {{ title }}
      </h1>
      <p class="min-w-0 truncate text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ subtitle }}
      </p>
    </div>
    <div class="flex-1" />
  </header>
</template>
