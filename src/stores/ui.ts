/**
 * UI 状态（Pinia）：当前分类 / 视图模式 / 搜索词 / 多页签工作区 / 设置页 / 全局 toast
 *
 * 页签关闭是**协商**而不是删除：所有关闭入口（关闭按钮、Ctrl+W、溢出菜单、关闭全部）
 * 都走 `requestClose` → 框架询问 owner（未保存内容/运行中任务）→ 允许才移除页签。
 * 低层移除函数不导出，调用方拿不到「绕过询问」的路径（类型层面即守住）。
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { negotiateToolClose, publishGlobalHidden } from '@/core/lifecycle'
import type { CloseIssue, CloseOutcome, CloseReason } from '@/core/lifecycle'

/** 待确认的关闭请求（有 owner 拒绝时需要用户决定放弃或取消） */
export interface ClosePrompt {
  /** 目标工具 id */
  toolId: string
  /** 拒绝原因（按 owner 上报，界面逐条展示） */
  blockers: CloseIssue[]
}

export const useUiStore = defineStore('ui', () => {
  /** 当前导航分类（"all" = 全部工具，"fav" = 我的收藏） */
  const activeCategory = ref<string>('all')
  /** 网格 / 列表视图 */
  const listView = ref(false)
  /** 顶栏搜索词 */
  const searchQuery = ref('')
  /** 侧栏折叠（标题栏按钮切换，内容区最大化） */
  const sidebarCollapsed = ref(false)
  /** 沉浸模式（隐藏工具标题栏 TopBar + 侧栏，退出时恢复侧栏展开） */
  const immersive = ref(false)
  /** 设置页开关（框架级整页模式：打开时右侧整体切换为设置页，不占工具页签） */
  const settingsOpen = ref(false)

  /** 打开设置页（整页铺满右侧，工具页签状态保留在后台） */
  function openSettings() {
    settingsOpen.value = true
  }

  /** 关闭设置页，回到之前的工具视图 */
  function closeSettings() {
    settingsOpen.value = false
  }

  /** 切换设置页（侧栏「设置」入口：打开态再点一次即退出） */
  function toggleSettings() {
    settingsOpen.value = !settingsOpen.value
  }

  /** 切换沉浸模式：进入收起侧栏，退出恢复侧栏展开 */
  function toggleImmersive() {
    if (immersive.value) {
      immersive.value = false
      sidebarCollapsed.value = false
    } else {
      immersive.value = true
      sidebarCollapsed.value = true
    }
  }

  /* ── 多页签工作区（工具以子页面形式打开，可切换/关闭，状态保持）── */
  /** 已打开的工具 id（页签顺序） */
  const openTabs = ref<string[]>([])
  /** 当前激活的工具 id（null = 工具库首页） */
  const activeTab = ref<string | null>(null)
  /** 主窗口是否隐藏到托盘（工具据此降频刷新；隐藏不等于断开） */
  const workspaceHidden = ref(false)
  /** 待用户确认的关闭请求（null = 无） */
  const closePrompt = ref<ClosePrompt | null>(null)
  /** 关闭过程中出现的清理失败提示（最近一次，界面 toast 用） */
  const lastCloseFailures = ref<CloseIssue[]>([])

  /** 打开工具页签：新页签插到首页后的第一位（已打开则仅激活）；自动退出设置页 */
  function openTool(id: string) {
    if (!openTabs.value.includes(id)) {
      openTabs.value.splice(1, 0, id)
    }
    activeTab.value = id
    settingsOpen.value = false
  }

  /**
   * 真正移除页签（**内部函数**）。
   *
   * 不导出：任何绕过 `requestClose` 的调用都会让「未保存内容」重新变成静默丢弃。
   */
  function removeTab(id: string) {
    const idx = openTabs.value.indexOf(id)
    if (idx < 0) return
    openTabs.value = openTabs.value.filter((x) => x !== id)
    if (activeTab.value === id) {
      activeTab.value = openTabs.value[idx - 1] ?? openTabs.value[0] ?? null
    }
  }

  /** 记录清理失败（界面据此提示，不允许静默） */
  function noteCloseFailures(failures: CloseIssue[]) {
    lastCloseFailures.value = failures
  }

  /**
   * 请求关闭工具页签：先协商，允许才移除。
   *
   * @param id 工具 id
   * @param reason 关闭原因（默认 'tab'）
   * @returns 协商结果（被拒绝时 `blockers` 已写入 `closePrompt`，界面弹确认）
   */
  async function requestClose(id: string, reason: CloseReason = 'tab'): Promise<CloseOutcome> {
    const outcome = await negotiateToolClose(id, reason)
    if (outcome.ok) {
      removeTab(id)
      noteCloseFailures(outcome.failures)
      return outcome
    }
    closePrompt.value = { toolId: id, blockers: outcome.blockers }
    return outcome
  }

  /** 用户在确认弹窗里选择「放弃并关闭」：跳过询问，直接清理并移除 */
  async function confirmClose(): Promise<CloseOutcome | null> {
    const prompt = closePrompt.value
    if (!prompt) return null
    closePrompt.value = null
    const outcome = await negotiateToolClose(prompt.toolId, 'tab', { force: true })
    removeTab(prompt.toolId)
    noteCloseFailures(outcome.failures)
    return outcome
  }

  /** 用户取消关闭（保留页签与内容） */
  function cancelClose() {
    closePrompt.value = null
  }

  /** 请求关闭全部工具页签；被拒绝的页签保留并进入确认流程 */
  async function requestCloseAll(reason: CloseReason = 'tab'): Promise<CloseOutcome[]> {
    const outcomes: CloseOutcome[] = []
    for (const id of [...openTabs.value]) {
      outcomes.push(await requestClose(id, reason))
    }
    return outcomes
  }

  /** 设置主窗口隐藏状态，并广播给所有工具（隐藏只降频，不改变连接与任务） */
  function setWorkspaceHidden(hidden: boolean) {
    if (workspaceHidden.value === hidden) return
    workspaceHidden.value = hidden
    publishGlobalHidden(hidden)
  }

  /** 回到工具库首页 */
  function goHome() {
    activeTab.value = null
  }

  /* ── 全局 toast（对齐原型 .toast，1600ms 自动消失）── */
  const toastMessage = ref('')
  const toastVisible = ref(false)
  let toastTimer: ReturnType<typeof setTimeout> | null = null

  function toast(msg: string) {
    toastMessage.value = msg
    toastVisible.value = true
    if (toastTimer) clearTimeout(toastTimer)
    toastTimer = setTimeout(() => {
      toastVisible.value = false
    }, 1600)
  }

  return {
    activeCategory,
    listView,
    searchQuery,
    sidebarCollapsed,
    immersive,
    toggleImmersive,
    settingsOpen,
    openSettings,
    closeSettings,
    toggleSettings,
    openTabs,
    activeTab,
    workspaceHidden,
    closePrompt,
    lastCloseFailures,
    openTool,
    requestClose,
    confirmClose,
    cancelClose,
    requestCloseAll,
    setWorkspaceHidden,
    goHome,
    toastMessage,
    toastVisible,
    toast,
  }
})
