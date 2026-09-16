/**
 * 数据管理状态（sync L2 · 导出/导入/空间）
 *
 * 为什么放在 store 而不是向导组件里：向导是弹窗，关闭即销毁；而「刚导出的包在哪、
 * 刚导入的空间叫什么、校验出来的包摘要」这些事实在关闭后仍要被设置页用（报告卡、空间列表）。
 * 放组件里就会出现「关掉弹窗，刚做的事没记录」。
 *
 * 密码不进 store：它只作为动作参数从组件传入、随调用结束即丢弃（Rust 侧同样不缓存）。
 * 失败一律：记录到诊断 + toast 可见原因，并返回 false 让向导留在原步（不静默、不假装成功）。
 */
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { ipc } from '@/core/ipc/ipc'
import { recordError } from '@/core/diagnostics/errors'
import { useUiStore } from '@/stores/ui'
import {
  buildSelection,
  initialChoice,
  type ExportChoice,
} from '@/features/settings/data/packSelection'
import type {
  ExportCatalog,
  ExportReport,
  ImportInspectResult,
  ImportPlanResult,
  ImportReport,
  SpaceSummary,
} from '@/core/ipc/contracts'

/** 出错时的统一处理：诊断留痕 + 用户可见提示，返回 false 供调用方留在原步 */
function failed(scope: string, error: unknown): false {
  const message = error instanceof Error ? error.message : String(error)
  recordError({ code: `data.${scope}.failed`, message, source: 'data' })
  useUiStore().toast(message)
  return false
}

export const useDataTransferStore = defineStore('dataTransfer', () => {
  /** 导出目录（当前空间可导出集合） */
  const catalog = ref<ExportCatalog | null>(null)
  /** 导出向导勾选态 */
  const choice = ref<ExportChoice>(initialChoice(null))
  /** 最近一次导出的报告（关闭向导后仍展示） */
  const exportReport = ref<ExportReport | null>(null)

  /** 导入：校验结果与计划 */
  const inspected = ref<ImportInspectResult | null>(null)
  const planned = ref<ImportPlanResult | null>(null)
  /** 最近一次导入的报告 */
  const importReport = ref<ImportReport | null>(null)
  /** 导入时选择要带入的数据集（缺省 = 包内全部可导入） */
  const importDatasets = ref<string[]>([])
  /** 导入确认（「我确认导入到一个新空间」）：与导出的敏感内容确认分开，语义不同 */
  const importAcknowledged = ref(false)

  /** 本机空间列表 */
  const spaces = ref<SpaceSummary[]>([])

  /** 正在跑的传输任务 id（界面据此在任务面板展示进度） */
  const taskId = ref('')
  /** 命令进行中标记（按钮禁用 + 转圈，避免重复提交） */
  const busy = ref('')

  const sourceSpaceName = computed(() => catalog.value?.sourceSpaceName ?? '')
  const canExport = computed(
    () =>
      choice.value.profileIds.length > 0 ||
      choice.value.includeFavorites ||
      choice.value.includeRecentTools
  )
  const activeSpace = computed(() => spaces.value.find((item) => item.active) ?? null)

  /** 读导出目录（打开导出向导时调用；失败返回 false 并已提示） */
  async function loadCatalog(): Promise<boolean> {
    busy.value = 'catalog'
    try {
      catalog.value = await ipc.dataExportCatalog()
      choice.value = initialChoice(catalog.value)
      return true
    } catch (error) {
      return failed('catalog', error)
    } finally {
      busy.value = ''
    }
  }

  /** 列出本机空间（设置页挂载时调用） */
  async function loadSpaces(): Promise<boolean> {
    busy.value = 'spaces'
    try {
      spaces.value = await ipc.dataSpacesList()
      return true
    } catch (error) {
      return failed('spaces', error)
    } finally {
      busy.value = ''
    }
  }

  /** 生成数据包（密码只在此处过一手） */
  async function exportPack(password: string, path: string): Promise<boolean> {
    if (!catalog.value) return failed('export', new Error('导出目录尚未加载，请重新打开向导'))
    busy.value = 'export'
    try {
      const started = await ipc.dataExportStart(
        buildSelection(catalog.value, choice.value),
        password,
        path
      )
      taskId.value = started.taskId
      exportReport.value = started.report
      useUiStore().toast(
        `已导出 ${Object.values(started.report.counts).reduce((sum, n) => sum + n, 0)} 条记录`
      )
      return true
    } catch (error) {
      return failed('export', error)
    } finally {
      busy.value = ''
    }
  }

  /** 校验数据包（只读：解密 + 清单校验 + 预览） */
  async function inspectPack(path: string, password: string): Promise<boolean> {
    busy.value = 'inspect'
    try {
      const result = await ipc.dataImportInspect(path, password)
      inspected.value = result
      planned.value = null
      importDatasets.value = [...result.defaults.datasets]
      return true
    } catch (error) {
      return failed('inspect', error)
    } finally {
      busy.value = ''
    }
  }

  /** 规划导入（生成新空间名与目标 id，尚未落盘） */
  async function planImport(spaceName: string, allowDuplicate: boolean): Promise<boolean> {
    if (!inspected.value) return failed('plan', new Error('请先校验数据包'))
    busy.value = 'plan'
    try {
      planned.value = await ipc.dataImportPlan(
        inspected.value.inspectId,
        { datasets: importDatasets.value },
        spaceName,
        allowDuplicate
      )
      return true
    } catch (error) {
      return failed('plan', error)
    } finally {
      busy.value = ''
    }
  }

  /** 提交导入（写新空间目录 + 登记索引） */
  async function commitImport(password: string): Promise<boolean> {
    if (!planned.value) return failed('commit', new Error('请先生成导入计划'))
    busy.value = 'commit'
    try {
      const result = await ipc.dataImportCommit(planned.value.planId, password)
      taskId.value = result.taskId
      importReport.value = result.report
      await loadSpaces()
      return true
    } catch (error) {
      return failed('commit', error)
    } finally {
      busy.value = ''
    }
  }

  /** 请求取消当前传输（无传输时后端返回 false，不作为错误） */
  async function cancelTransfer(): Promise<boolean> {
    try {
      const result = await ipc.dataTransferCancel(taskId.value || null)
      if (result.cancelled) useUiStore().toast('已请求取消，会在安全点停止')
      return result.cancelled
    } catch (error) {
      return failed('cancel', error)
    }
  }

  /** 切换活动空间（写指针，重启后生效） */
  async function switchSpace(spaceId: string): Promise<boolean> {
    busy.value = 'switch'
    try {
      const result = await ipc.dataSpaceSwitch(spaceId)
      useUiStore().toast(
        result.restartRequired
          ? `已切换到「${result.name}」，重启后生效`
          : `已切换到「${result.name}」`
      )
      await loadSpaces()
      return true
    } catch (error) {
      return failed('switch', error)
    } finally {
      busy.value = ''
    }
  }

  /** 关闭导出向导后重置勾选（报告保留） */
  function resetChoice(): void {
    choice.value = initialChoice(catalog.value)
  }

  /** 关闭导入向导后清掉本次校验/计划（报告保留） */
  function resetImport(): void {
    inspected.value = null
    planned.value = null
    importDatasets.value = []
    importAcknowledged.value = false
  }

  return {
    catalog,
    choice,
    exportReport,
    inspected,
    planned,
    importReport,
    importDatasets,
    importAcknowledged,
    spaces,
    taskId,
    busy,
    sourceSpaceName,
    canExport,
    activeSpace,
    loadCatalog,
    loadSpaces,
    exportPack,
    inspectPack,
    planImport,
    commitImport,
    cancelTransfer,
    switchSpace,
    resetChoice,
    resetImport,
  }
})
