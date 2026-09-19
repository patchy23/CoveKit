/** 端口快照、分页与按可见性暂停的刷新，确认关闭期间冻结查询快照。 */
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { createScope, type Scope } from '@/core/lifecycle/scope'
import { useToolScope } from '@/core/lifecycle/useToolLifecycle'
import { isDesktopRuntime } from '@/core/platform/window'
import { IpcError } from '@/core/ipc/ipc'
import { useUiStore } from '@/stores/ui'
import type { PortEntry, PortSnapshot } from './contracts'
import { endpointOf, filterEntries, filterError, type PortFilter } from './entries'
import { ipc } from './ipc'

function message(cause: unknown): string {
  if (cause instanceof IpcError) return cause.message.replace(/^\[[^\]]+\]\s*/, '')
  return cause instanceof Error ? cause.message : String(cause)
}

export function usePortViewer() {
  const ui = useUiStore()
  const { scope, visibility } = useToolScope('port-viewer')
  const desktop = isDesktopRuntime()
  const supported = ref<boolean | null>(null)
  const checking = ref(false),
    busy = ref(false),
    auto = ref(false)
  const snapshot = ref<PortSnapshot | null>(null)
  const error = ref(''),
    queriedAt = ref('')
  const filter = reactive<PortFilter>({
    search: '',
    by: 'port',
    protocol: 'all',
    view: 'listeners',
  })
  const validation = computed(() => filterError(filter))
  const filtered = computed(() => filterEntries(snapshot.value?.entries ?? [], filter))
  const page = ref(1)
  const totalPages = computed(() => Math.max(1, Math.ceil(filtered.value.length / 100)))
  const pageEntries = computed(() => filtered.value.slice((page.value - 1) * 100, page.value * 100))
  const closeTarget = ref<PortEntry | null>(null),
    closeError = ref(''),
    closing = ref(false)
  const available = computed(() => desktop && supported.value === true)
  const visible = computed(
    () => visibility.value.active && !visibility.value.covered && !visibility.value.hidden
  )
  let polling: Scope | null = null

  watch(filter, () => {
    page.value = 1
  })
  watch(totalPages, (total) => {
    page.value = Math.min(page.value, total)
  })

  async function query() {
    if (!available.value || busy.value || closing.value || closeTarget.value || scope.disposed)
      return
    busy.value = true
    error.value = ''
    try {
      const next = await ipc.query()
      if (scope.disposed) return
      snapshot.value = next
      queriedAt.value = new Date().toLocaleTimeString()
    } catch (cause) {
      if (scope.disposed) return
      error.value = `${message(cause)}${auto.value ? '；自动刷新已暂停，请手动重试' : ''}`
      auto.value = false
    } finally {
      if (!scope.disposed) busy.value = false
    }
  }

  async function initialize() {
    if (!desktop || checking.value || scope.disposed) return
    checking.value = true
    error.value = ''
    try {
      const value = await ipc.supported()
      if (scope.disposed) return
      supported.value = value
      if (value) await query()
    } catch (cause) {
      if (!scope.disposed) error.value = `读取平台支持状态失败：${message(cause)}`
    } finally {
      if (!scope.disposed) checking.value = false
    }
  }

  const canPoll = computed(
    () => auto.value && available.value && visible.value && !closeTarget.value && !closing.value
  )
  watch(canPoll, (enabled) => {
    void polling?.dispose()
    polling = null
    if (!enabled || scope.disposed) return
    polling = createScope('port-viewer.refresh')
    polling.interval(() => {
      void query()
    }, 3000)
    void query()
  })
  scope.addDispose(async () => {
    await polling?.dispose()
    polling = null
  })

  function requestClose(entry: PortEntry) {
    if (
      !entry.startedAt ||
      entry.pid <= 4 ||
      busy.value ||
      error.value ||
      closing.value ||
      scope.disposed
    )
      return
    closeError.value = ''
    closeTarget.value = { ...entry }
  }
  function cancelClose() {
    if (closing.value) return
    closeTarget.value = null
    closeError.value = ''
  }
  async function confirmClose() {
    const target = closeTarget.value
    if (!target?.startedAt || closing.value || scope.disposed) return
    closing.value = true
    closeError.value = ''
    try {
      await ipc.terminate({ endpoint: endpointOf(target), startedAt: target.startedAt })
      if (scope.disposed) return
      closeTarget.value = null
      closing.value = false
      ui.toast(`进程 ${target.processName || target.pid} 已关闭`)
      await query()
    } catch (cause) {
      if (!scope.disposed) closeError.value = message(cause)
    } finally {
      if (!scope.disposed) closing.value = false
    }
  }

  onMounted(initialize)
  return {
    desktop,
    supported,
    available,
    checking,
    busy,
    auto,
    snapshot,
    error,
    queriedAt,
    filter,
    validation,
    filtered,
    page,
    totalPages,
    pageEntries,
    visible,
    closeTarget,
    closeError,
    closing,
    query,
    initialize,
    requestClose,
    cancelClose,
    confirmClose,
  }
}
