<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { UiTooltip } from '@/core/ui'
/**
 * ToolWorkspace · 多页签工作区（用户决策：工具以子页面形式打开，替代弹窗）
 * 页签条 + 内容区：首页（工具库）/ 各工具页签（v-show 保持组件状态，切换不销毁）。
 * 页签过多时：新页签在首页后第一位，超出显示宽度的页签收纳进「···」下拉。
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { getTool } from '@/core/registry/toolRegistry'
import { publishToolVisibility, watchToolCloseState, type ToolCloseState } from '@/core/lifecycle'
import { UiTabsOverflowMenu } from '@/core/ui'
import { useTabsOverflow } from '@/core/ui/useTabsOverflow'
import AppIcon from '@/features/ui/AppIcon.vue'
import RecentStrip from '@/features/recent/RecentStrip.vue'
import ToolGrid from '@/features/grid/ToolGrid.vue'
import ToolList from '@/features/grid/ToolList.vue'
import ToolHost from './ToolHost.vue'
import CloseConfirmDialog from './CloseConfirmDialog.vue'
import { useToolsStore } from '@/stores/tools'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
const tools = useToolsStore()

const hasTools = computed(() => tools.tools.length > 0)
const searching = computed(() => ui.searchQuery.trim().length > 0)

/** 组件加载器（宿主负责异步加载、加载态与失败重试） */
function loaderFor(id: string) {
  return getTool(id)?.component ?? (() => Promise.reject(new Error(`未登记的工具：${id}`)))
}

/* ── 页签运行中标记 + 工具可见性分发 ── */
/** 工具页签只呈现运行状态；未保存提示由内部文档页签与关闭协商负责。 */
const tabState = ref<Record<string, ToolCloseState>>({})
/** 每个页签的关闭状态退订函数 */
const stateStops = new Map<string, () => void>()

/** 按当前打开的页签订阅/退订关闭状态变化，避免订阅长期挂在已关闭的工具上 */
watch(
  () => [...ui.openTabs],
  (ids) => {
    for (const id of ids) {
      if (stateStops.has(id)) continue
      stateStops.set(
        id,
        watchToolCloseState(id, (state) => {
          tabState.value = { ...tabState.value, [id]: state }
        })
      )
    }
    for (const [id, stop] of [...stateStops]) {
      if (ids.includes(id)) continue
      stop()
      stateStops.delete(id)
      const rest = { ...tabState.value }
      delete rest[id]
      tabState.value = rest
    }
  },
  { immediate: true }
)

/**
 * 向工具广播可见性：激活页签 / 被设置页覆盖 / 窗口隐藏分别可见。
 *
 * 插件据此决定「降频刷新」还是「保持心跳」——失焦不等于断开，隐藏不等于停止。
 */
watch(
  () => [ui.activeTab, ui.settingsOpen, ui.workspaceHidden, ui.openTabs.length] as const,
  () => {
    for (const id of ui.openTabs) {
      publishToolVisibility(id, {
        active: ui.activeTab === id,
        covered: ui.settingsOpen,
        hidden: ui.workspaceHidden,
      })
    }
  },
  { immediate: true }
)

// 清理失败必须被看见：关页签时插件清理出错，用 toast 报出来（禁止静默）
watch(
  () => ui.lastCloseFailures,
  (failures) => {
    if (failures.length === 0) return
    const owners = failures.map((item) => item.owner).join('、')
    ui.toast(`关闭时有清理失败：${owners}`)
  }
)

/** 某页签是否有运行中的任务（页签标记用） */
function tabRunning(id: string) {
  return tabState.value[id]?.running ?? false
}

function tabTitle(id: string) {
  return getTool(id)?.name ?? id
}

function tabIcon(id: string) {
  return getTool(id)?.icon ?? 'all'
}

/** 从溢出下拉打开页签 */
function openHiddenTool(id: string) {
  ui.openTool(id)
}

/* ── 页签键盘快捷键：Ctrl+W 关闭当前页签，Ctrl(+Shift)+Tab 左右循环切换 ── */
function onTabKeydown(event: KeyboardEvent) {
  if (!event.ctrlKey) return
  if (event.key === 'w' || event.key === 'W') {
    // 首页不可关；无工具页签时不拦截（避免与系统/其他快捷键语义冲突）
    if (!ui.activeTab) return
    event.preventDefault()
    void ui.requestClose(ui.activeTab)
    return
  }
  if (event.key === 'Tab' && ui.openTabs.length > 1) {
    event.preventDefault()
    // 候选序列 = 首页('') + 打开的工具页签
    const seq: string[] = ['', ...ui.openTabs]
    const cur = seq.indexOf(ui.activeTab ?? '')
    const next = event.shiftKey ? (cur - 1 + seq.length) % seq.length : (cur + 1) % seq.length
    const target = seq[next]
    if (target) ui.openTool(target)
    else ui.goHome()
  }
}

onMounted(() => window.addEventListener('keydown', onTabKeydown))
onUnmounted(() => {
  window.removeEventListener('keydown', onTabKeydown)
  for (const stop of stateStops.values()) stop()
  stateStops.clear()
})

/** 溢出下拉里关闭页签：与页签关闭走同一条协商路径 */
function onOverflowClose(id: string) {
  void ui.requestClose(id)
}

/* ── 页签溢出收纳：按页签条可用宽度逐页签估算，放得下几个显示几个，其余进「···」下拉 ── */
// extra=60：图标15 + 间距12 + 关闭18 + padding 15；maxLabel=120 与页签 max-w-[120px] 对齐。
// reserved=150：「首页」页签 + 「···」触发器 + 尾部留白，避免溢出时三点不出现。
const tabBar = ref<HTMLElement | null>(null)
const tabItems = computed(() =>
  ui.openTabs.map((id) => ({ value: id, label: tabTitle(id), closable: true }))
)
const activeTabValue = computed(() => ui.activeTab ?? '')
const { visibleItems, hiddenItems } = useTabsOverflow(tabBar, tabItems, activeTabValue, 150, {
  extra: 60,
  maxLabel: 120,
})
/** 页签条直接显示的页签 id */
const visibleTabs = computed(() => visibleItems.value.map((t) => t.value))
/** 收纳进下拉的页签（UiTabsOverflowMenu items 形态） */
const hiddenTabItems = computed(() => hiddenItems.value)
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <!-- 页签条 -->
    <div
      ref="tabBar"
      class="flex shrink-0 items-center gap-[2px] overflow-hidden border-b border-border bg-surface-muted px-sm dark:border-border-dark dark:bg-surface-muted-dark"
    >
      <UiTooltip content="回到工具库首页">
        <button
          class="relative flex h-[38px] shrink-0 items-center gap-[8px] rounded-t-[8px] px-[14px] text-body font-medium transition-colors"
          :class="
            ui.activeTab === null
              ? 'bg-surface text-primary dark:bg-surface-dark dark:text-primary-dark'
              : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
          "
          aria-label="回到工具库首页"
          @click="ui.goHome()"
        >
          <!-- 选中指示器：纯横线（独立元素，不随圆角弯曲） -->
          <span
            v-if="ui.activeTab === null"
            class="absolute inset-x-0 top-0 h-[2px] bg-tertiary-strong dark:bg-tertiary-dark"
          />
          <AppIcon name="all" :size="15" />
          首页
        </button>
      </UiTooltip>
      <div
        v-for="id in visibleTabs"
        :key="id"
        class="group relative flex h-[38px] shrink-0 cursor-pointer items-center gap-[8px] rounded-t-[8px] px-[12px] text-body font-medium transition-colors"
        :class="
          ui.activeTab === id
            ? 'bg-surface text-primary dark:bg-surface-dark dark:text-primary-dark'
            : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
        "
        @click="ui.openTool(id)"
      >
        <!-- 选中指示器：纯横线（独立元素，不随圆角弯曲） -->
        <span
          v-if="ui.activeTab === id"
          class="absolute inset-x-0 top-0 h-[2px] bg-tertiary-strong dark:bg-tertiary-dark"
        />
        <AppIcon
          :name="tabIcon(id)"
          :size="15"
          class="text-tertiary-strong dark:text-tertiary-dark"
        />
        <span class="max-w-[120px] truncate">{{ tabTitle(id) }}</span>
        <!-- 工具页签只显示运行状态，未保存内容仍由关闭协商保护。 -->
        <UiTooltip v-if="tabRunning(id)" content="任务进行中">
          <span
            class="h-[6px] w-[6px] shrink-0 rounded-full bg-tertiary-strong dark:bg-tertiary-dark"
          />
        </UiTooltip>
        <UiTooltip content="关闭页签">
          <button
            class="grid h-[18px] w-[18px] shrink-0 place-items-center rounded-[4px] text-text-muted opacity-0 transition-opacity hover:bg-border hover:text-tertiary-strong group-hover:opacity-100 dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-tertiary-dark"
            aria-label="关闭页签"
            @click.stop="ui.requestClose(id)"
          >
            <AppIcon name="close" :size="11" />
          </button>
        </UiTooltip>
      </div>

      <!-- 溢出收纳：公共组件（··· 触发器 + 下拉） -->
      <UiTabsOverflowMenu
        v-if="hiddenTabItems.length"
        :items="hiddenTabItems"
        :model-value="ui.activeTab ?? ''"
        @select="openHiddenTool"
        @close="onOverflowClose"
      />
      <div class="flex-1" />
    </div>

    <!-- 内容区 -->
    <UiScrollArea as-child axis="vertical">
      <div class="min-h-0 flex-1 px-md py-md">
        <!-- 首页：工具库（列表模式下限高自滚，滚动条收在列表边框内；卡片模式仍随内容区滚动） -->
        <div
          v-show="ui.activeTab === null"
          :class="ui.listView ? 'flex h-full min-h-0 flex-col' : ''"
        >
          <RecentStrip />
          <div class="mb-[12px] flex items-center gap-sm">
            <h2 class="text-h2 font-bold tracking-[-0.01em] dark:text-primary-dark">工具列表</h2>
            <span class="text-body-sm text-text-muted dark:text-text-muted-dark"
              >{{ tools.filtered.length }} 个</span
            >
            <div class="flex-1" />
            <!-- 视图切换（卡片/列表，默认卡片；自顶栏迁入） -->
            <div
              class="flex items-center gap-[2px] rounded-md border border-border bg-neutral p-[2px] dark:border-border-dark dark:bg-neutral-dark"
            >
              <UiTooltip content="卡片模式">
                <button
                  class="grid h-[24px] w-[28px] place-items-center rounded-[5px] transition-colors duration-100"
                  :class="
                    !ui.listView
                      ? 'bg-surface text-tertiary-strong shadow-sm dark:bg-surface-dark dark:text-tertiary-dark'
                      : 'text-text-muted hover:text-primary dark:text-text-muted-dark dark:hover:text-primary-dark'
                  "
                  aria-label="卡片模式"
                  @click="ui.listView = false"
                >
                  <AppIcon name="grid" :size="14" />
                </button>
              </UiTooltip>
              <UiTooltip content="列表模式">
                <button
                  class="grid h-[24px] w-[28px] place-items-center rounded-[5px] transition-colors duration-100"
                  :class="
                    ui.listView
                      ? 'bg-surface text-tertiary-strong shadow-sm dark:bg-surface-dark dark:text-tertiary-dark'
                      : 'text-text-muted hover:text-primary dark:text-text-muted-dark dark:hover:text-primary-dark'
                  "
                  aria-label="列表模式"
                  @click="ui.listView = true"
                >
                  <AppIcon name="list" :size="14" />
                </button>
              </UiTooltip>
            </div>
          </div>
          <ToolGrid v-if="tools.filtered.length && !ui.listView" />
          <ToolList v-else-if="tools.filtered.length && ui.listView" class="min-h-0 flex-1" />
          <div v-else class="flex flex-col items-center justify-center py-[96px] text-center">
            <div
              class="grid h-11 w-11 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
            >
              <AppIcon
                :name="searching ? 'search' : 'all'"
                :size="22"
                class="text-tertiary-strong dark:text-tertiary-dark"
              />
            </div>
            <p class="mt-md text-h2 font-bold dark:text-primary-dark">
              {{ searching ? '未找到匹配工具' : hasTools ? '该分类暂无工具' : '暂无工具' }}
            </p>
            <p class="mt-xs text-body-sm text-text-muted dark:text-text-muted-dark">
              {{
                searching
                  ? '换个关键词试试'
                  : hasTools
                    ? '工具将在此分类上线'
                    : '首批 8 个文本工具将在 M1 上线'
              }}
            </p>
          </div>
        </div>

        <!-- 工具页签（v-show 保持状态，切换不销毁；h-full 让工具可内部滚动） -->
        <div v-for="id in ui.openTabs" v-show="ui.activeTab === id" :key="id" class="h-full">
          <ToolHost :tool-id="id" :title="tabTitle(id)" :loader="loaderFor(id)" />
        </div>
      </div>
    </UiScrollArea>

    <!-- 关闭确认：有未保存内容或运行中任务时先问用户（T10-2） -->
    <CloseConfirmDialog />
  </div>
</template>
