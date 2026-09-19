/** 单文件查询状态、原生拖放订阅及迟到响应隔离；不持久化用户路径。 */
import { computed, onMounted, ref, watch, type Ref } from 'vue'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import { useToolScope } from '@/core/lifecycle/useToolLifecycle'
import { isDesktopRuntime } from '@/core/platform/window'
import type { FileLockResult, FileProcess } from './contracts'
import { ipc } from './ipc'

/** 生命周期随工具卸载结束；隐藏页签不接收全窗口文件拖放。 */
export function useFileLock(page: Ref<HTMLElement | null>) {
  const { scope, visibility } = useToolScope('file-lock')
  const desktop = isDesktopRuntime()
  const supported = ref<boolean | null>(null)
  const checking = ref(false)
  const path = ref('')
  const busy = ref(false)
  const picking = ref(false)
  const error = ref('')
  const dropError = ref('')
  const dragging = ref(false)
  const result = ref<FileLockResult | null>(null)
  const queriedAt = ref('')
  const closeTarget = ref<{ path: string; process: FileProcess } | null>(null)
  const closing = ref(false)
  const closeError = ref('')
  const notice = ref('')
  let revision = 0
  // 框架 hidden 同时包含窗口失焦，适合后台降频但不能用于外部拖放。
  // 从资源管理器拖入时允许窗口失焦；实际页面隐藏在原生事件到达时另行检查。
  const visible = computed(() => {
    const state = visibility.value
    return state.active && !state.covered
  })
  const available = computed(() => desktop && supported.value === true)

  watch(
    path,
    () => {
      revision += 1
      result.value = null
      error.value = ''
      queriedAt.value = ''
      notice.value = ''
      if (!closing.value) closeTarget.value = null
    },
    { flush: 'sync' }
  )
  watch(visible, () => {
    dragging.value = false
  })

  async function query() {
    if (
      !available.value ||
      busy.value ||
      picking.value ||
      closing.value ||
      closeTarget.value ||
      scope.disposed
    )
      return
    // 支持资源管理器“复制文件地址”的双引号，不改写路径内部空格。
    const raw = path.value.trim()
    const target = raw.startsWith('"') && raw.endsWith('"') ? raw.slice(1, -1) : raw
    path.value = target
    result.value = null
    queriedAt.value = ''
    error.value = ''
    if (!target) {
      error.value = '请选择文件或输入完整的绝对路径'
      return
    }
    const request = ++revision
    busy.value = true
    try {
      const next = await ipc.query(target)
      if (scope.disposed || request !== revision) return
      result.value = next
      queriedAt.value = new Date().toLocaleTimeString()
    } catch (cause) {
      if (!scope.disposed && request === revision) error.value = String(cause)
    } finally {
      if (!scope.disposed) busy.value = false
    }
  }

  async function chooseFile() {
    if (
      !available.value ||
      busy.value ||
      picking.value ||
      closing.value ||
      closeTarget.value ||
      scope.disposed
    )
      return
    picking.value = true
    error.value = ''
    let selected: string | string[] | null = null
    try {
      selected = await open({ title: '选择要查询占用的文件', directory: false, multiple: false })
    } catch (cause) {
      if (!scope.disposed) error.value = `选择文件失败：${String(cause)}`
    } finally {
      if (!scope.disposed) picking.value = false
    }
    if (typeof selected === 'string' && !scope.disposed) {
      path.value = selected
      await query()
    }
  }

  function requestClose(process: FileProcess) {
    if (!result.value || busy.value || closing.value || scope.disposed) return
    closeError.value = ''
    notice.value = ''
    closeTarget.value = { path: result.value.path, process: { ...process } }
  }

  function cancelClose() {
    if (closing.value) return
    closeTarget.value = null
    closeError.value = ''
  }

  async function confirmClose() {
    const target = closeTarget.value
    if (!target || closing.value || scope.disposed) return
    closing.value = true
    closeError.value = ''
    try {
      await ipc.terminate({
        path: target.path,
        pid: target.process.pid,
        startedAt: target.process.startedAt,
      })
      if (scope.disposed) return
      closeTarget.value = null
      closing.value = false
      notice.value = `进程 ${target.process.processName || target.process.appName || target.process.pid} 已关闭`
      await query()
    } catch (cause) {
      if (!scope.disposed) closeError.value = String(cause)
    } finally {
      if (!scope.disposed) closing.value = false
    }
  }

  function inside(position: { x: number; y: number }) {
    const bounds = page.value?.getBoundingClientRect()
    if (!bounds || bounds.width === 0 || bounds.height === 0) return false
    const scale = window.devicePixelRatio || 1
    const x = position.x / scale
    const y = position.y / scale
    return x >= bounds.left && x <= bounds.right && y >= bounds.top && y <= bounds.bottom
  }

  async function initialize() {
    if (!desktop || checking.value || supported.value !== null || scope.disposed) return
    checking.value = true
    error.value = ''
    try {
      const value = await ipc.supported()
      if (scope.disposed) return
      supported.value = value
      if (!value) return
      try {
        const stop = await getCurrentWebview().onDragDropEvent(({ payload }) => {
          if (
            scope.disposed ||
            !visible.value ||
            document.visibilityState === 'hidden' ||
            busy.value ||
            picking.value ||
            closing.value ||
            closeTarget.value ||
            payload.type === 'leave'
          ) {
            dragging.value = false
            return
          }
          const within = inside(payload.position)
          dragging.value = within && payload.type !== 'drop'
          if (payload.type !== 'drop' || !within) return
          if (payload.paths.length !== 1) {
            error.value = '请一次拖入一个文件，暂不支持批量查询'
            return
          }
          path.value = payload.paths[0]!
          void query()
        })
        // 注册晚于卸载时立即解绑，避免已关闭页签仍接收文件路径。
        if (scope.disposed) stop()
        else scope.addDispose(stop)
      } catch (cause) {
        if (!scope.disposed)
          dropError.value = `文件拖放不可用，请使用选择文件或输入路径：${String(cause)}`
      }
    } catch (cause) {
      if (!scope.disposed) error.value = `读取平台支持状态失败：${String(cause)}`
    } finally {
      if (!scope.disposed) checking.value = false
    }
  }

  onMounted(initialize)
  return {
    desktop,
    supported,
    checking,
    available,
    path,
    busy,
    picking,
    error,
    dropError,
    dragging,
    result,
    queriedAt,
    query,
    chooseFile,
    initialize,
    closeTarget,
    closing,
    closeError,
    notice,
    requestClose,
    cancelClose,
    confirmClose,
  }
}
