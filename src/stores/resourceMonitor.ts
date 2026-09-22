/** 应用级监测会话；采样独立于侧栏、浮层和设置页的显示生命周期。 */
import { computed, onScopeDispose, ref, shallowRef, watch } from 'vue'
import { defineStore } from 'pinia'
import { isTauri } from '@tauri-apps/api/core'
import { ipc } from '@/core/ipc/ipc'
import { getTools } from '@/core/registry/toolRegistry'
import {
  configureToolMetrics,
  setMonitorCommands,
  toolMetricSnapshot,
} from '@/core/resourceMonitor/metrics'
import {
  createResourcePoller,
  resourceTotals,
  type TimedSnapshot,
} from '@/core/resourceMonitor/sampling'
import { useSettingsStore } from './settings'

export const useResourceMonitorStore = defineStore('resourceMonitor', () => {
  const settings = useSettingsStore()
  const enabled = computed(() => !!settings.settings.resourceMonitorEnabled)
  const selected = computed(() => settings.settings.resourceMonitorTools ?? [])
  const catalog = ref<{ name: string; doc: string; toolId: string }[]>([])
  const catalogError = ref('')
  const catalogLoading = ref(false)
  const snapshot = shallowRef<TimedSnapshot>()
  const totals = shallowRef<ReturnType<typeof resourceTotals>>()
  const error = ref('')
  const updatedAt = ref<number>()
  const peakMemory = ref(0)
  const peakCpu = ref<number | null>(null)
  const details = ref<({ id: string } & ReturnType<typeof toolMetricSnapshot>)[]>([])
  const focusSettings = ref(false)
  let catalogPending: Promise<void> | undefined

  const supportedTools = computed(() => {
    const ids = new Set(catalog.value.map((entry) => entry.toolId))
    return getTools().filter((tool) => ids.has(tool.id))
  })

  async function loadCatalog() {
    if (catalog.value.length) return
    if (catalogPending) return catalogPending
    catalogLoading.value = true
    catalogError.value = ''
    catalogPending = (async () => {
      try {
        if (!isTauri()) throw new Error('工具统计需要在桌面应用中使用')
        catalog.value = await ipc.frameworkCommandsList()
        setMonitorCommands(catalog.value)
      } catch (cause) {
        catalogError.value = String(cause)
      } finally {
        catalogPending = undefined
        catalogLoading.value = false
      }
    })()
    return catalogPending
  }

  const poller = createResourcePoller(
    async () => {
      if (!isTauri()) throw new Error('应用资源采样仅在桌面环境可用')
      return ipc.resourceMonitorSnapshot()
    },
    (value) => {
      const current = { value, time: performance.now() }
      totals.value = resourceTotals(current, snapshot.value)
      snapshot.value = current
      updatedAt.value = Date.now()
      error.value = ''
      peakMemory.value = Math.max(peakMemory.value, totals.value.resident)
      if (totals.value.cpu !== null) peakCpu.value = Math.max(peakCpu.value ?? 0, totals.value.cpu)
      details.value = selected.value.map((id) => ({ id, ...toolMetricSnapshot(id) }))
    },
    (cause) => {
      error.value = String(cause)
    }
  )

  watch(
    [enabled, selected],
    ([active, ids]) => {
      configureToolMetrics(active ? ids : [])
      details.value = active ? ids.map((id) => ({ id, ...toolMetricSnapshot(id) })) : []
    },
    { immediate: true }
  )
  watch(
    enabled,
    (active) => {
      if (active) {
        void loadCatalog()
        poller.start()
      } else {
        poller.stop()
        snapshot.value = undefined
        totals.value = undefined
        updatedAt.value = undefined
        error.value = ''
        peakMemory.value = 0
        peakCpu.value = null
      }
    },
    { immediate: true }
  )
  onScopeDispose(() => {
    poller.stop()
    configureToolMetrics([])
  })

  return {
    enabled,
    selected,
    supportedTools,
    loadCatalog,
    catalogError,
    catalogLoading,
    snapshot,
    totals,
    error,
    updatedAt,
    peakMemory,
    peakCpu,
    details,
    focusSettings,
  }
})
