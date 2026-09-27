/**
 * 远程目录导航：目录快照缓存、后退历史、排序与快速切换竞态守卫。
 * 快照整批替换，作用域关闭清空缓存并使在途响应失效。
 */
import { computed, ref, shallowRef, onScopeDispose } from 'vue'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { RemoteFile } from '../contracts'

// 排序规则固定；复用 Collator，避免每次比较都解析 localeCompare 的区域与选项。
const nameCollator = new Intl.Collator('zh-CN', { numeric: true, sensitivity: 'base' })
const DIRECTORY_IDLE_MS = 10 * 60 * 1000

export function useRemoteDirectory(deps: {
  sessionId: () => string | undefined
  /** 切换目录时清空选择（外部注入） */
  onNavigate?: () => void
}) {
  const ui = useUiStore()

  const currentPath = ref('/')
  const directoryHistory = ref<string[]>([])
  const DIRECTORY_HISTORY_LIMIT = 20
  // 列表由 IPC/缓存整批替换，文件操作后刷新；不在快照上原地修改单个文件。
  const files = shallowRef<RemoteFile[]>([])
  const sortKey = ref<'name' | 'modifiedAt'>('name')
  const sortDirection = ref<'asc' | 'desc'>('asc')
  const directoryCache = new Map<string, { files: RemoteFile[]; lastUsed: number }>()
  let displayedKey: string | undefined
  let expiryTimer: ReturnType<typeof setTimeout> | undefined
  /** 目录缓存上限：超出时淘汰最旧条目（Map 保持插入序），防长期浏览无限增长 */
  const DIRECTORY_CACHE_LIMIT = 100
  /** 目录请求序号：快速连续切换时只认最后一次请求的目录（竞态守卫） */
  let navigateSeq = 0
  let disposed = false
  onScopeDispose(() => {
    disposed = true
    navigateSeq++
    clearExpiryTimer()
    displayedKey = undefined
    directoryCache.clear()
    files.value = []
    directoryHistory.value = []
  })

  function clearExpiryTimer() {
    if (expiryTimer !== undefined) clearTimeout(expiryTimer)
    expiryTimer = undefined
  }

  /** 仅保留最近的闲置过期任务；当前显示快照不淘汰，也不触发网络刷新。 */
  function expireIdle() {
    clearExpiryTimer()
    const now = performance.now()
    let nextExpiry = Infinity
    for (const [key, entry] of directoryCache) {
      if (key === displayedKey) continue
      const remaining = entry.lastUsed + DIRECTORY_IDLE_MS - now
      if (remaining <= 0) directoryCache.delete(key)
      else nextExpiry = Math.min(nextExpiry, remaining)
    }
    if (!disposed && Number.isFinite(nextExpiry))
      expiryTimer = setTimeout(expireIdle, Math.ceil(nextExpiry))
  }

  /** 写入目录缓存（带淘汰：同 key 刷新位置，超限删最旧） */
  function cacheDirectory(key: string, list: RemoteFile[]) {
    const now = performance.now()
    if (displayedKey !== key && displayedKey) {
      const previous = directoryCache.get(displayedKey)
      if (previous) previous.lastUsed = now
    }
    displayedKey = key
    files.value = list
    if (directoryCache.has(key)) directoryCache.delete(key)
    directoryCache.set(key, { files: list, lastUsed: now })
    if (directoryCache.size > DIRECTORY_CACHE_LIMIT) {
      const oldest = directoryCache.keys().next().value
      if (oldest !== undefined) directoryCache.delete(oldest)
    }
    expireIdle()
  }

  const parentPath = computed(() => {
    const p = currentPath.value
    if (p === '/') return null
    const idx = p.lastIndexOf('/')
    return idx <= 0 ? '/' : p.slice(0, idx)
  })

  const sortedFiles = computed(() => {
    const key = sortKey.value
    const direction = sortDirection.value
    return [...files.value].sort((left, right) => {
      if (left.isDir !== right.isDir) return left.isDir ? -1 : 1
      const comparison =
        key === 'name'
          ? nameCollator.compare(left.name, right.name)
          : left.modifiedAt - right.modifiedAt
      return direction === 'asc' ? comparison : -comparison
    })
  })

  function cacheKey(connectionId: string, path: string) {
    return `${connectionId}\u0000${path}`
  }

  async function navigate(path: string, force = false, recordHistory = true) {
    if (disposed) return
    const connectionId = deps.sessionId()
    if (!connectionId) return
    // 竞态守卫：序号单调递增，慢返回的旧目录请求直接丢弃（不再回写 files/缓存）
    const seq = ++navigateSeq
    const previousPath = currentPath.value
    if (recordHistory && path !== previousPath) {
      directoryHistory.value = [...directoryHistory.value, previousPath].slice(
        -DIRECTORY_HISTORY_LIMIT
      )
    }
    currentPath.value = path
    deps.onNavigate?.()
    const key = cacheKey(connectionId, path)
    // 后台 WebView 可能延后定时器，命中前再核对期限，不能复用已过期的闲置快照。
    expireIdle()
    const cached = directoryCache.get(key)
    if (cached && !force) {
      cacheDirectory(key, cached.files)
      return
    }
    try {
      const r = await ipc.sshFileList(connectionId, path)
      if (seq !== navigateSeq) return // 已有更新的目录请求，丢弃本次结果
      if (deps.sessionId() !== connectionId) return
      if (r.ok) {
        cacheDirectory(key, r.files)
      } else {
        ui.toast(`读取目录失败：${r.error ?? '未知错误'}`)
      }
    } catch (e) {
      if (seq !== navigateSeq) return
      ui.toast(`读取目录失败：${e}`)
    }
  }

  function refreshCurrent() {
    return navigate(currentPath.value, true)
  }

  function changeSort(key: 'name' | 'modifiedAt') {
    if (sortKey.value === key) sortDirection.value = sortDirection.value === 'asc' ? 'desc' : 'asc'
    else {
      sortKey.value = key
      sortDirection.value = key === 'name' ? 'asc' : 'desc'
    }
  }

  function navigateUp() {
    if (parentPath.value) navigate(parentPath.value)
  }

  function navigateBack() {
    const previousPath = directoryHistory.value[directoryHistory.value.length - 1]
    if (!previousPath) return
    directoryHistory.value = directoryHistory.value.slice(0, -1)
    void navigate(previousPath, false, false)
  }

  /** 切换连接：清空全部导航状态并从根目录重来 */
  function reset(sessionId: string | undefined, path = '/') {
    clearExpiryTimer()
    displayedKey = undefined
    files.value = []
    directoryCache.clear()
    directoryHistory.value = []
    navigateSeq++
    currentPath.value = path
    if (sessionId) void navigate(path, true, false)
  }

  return {
    currentPath,
    directoryHistory,
    files,
    sortKey,
    sortDirection,
    parentPath,
    sortedFiles,
    navigate,
    refreshCurrent,
    changeSort,
    navigateUp,
    navigateBack,
    reset,
  }
}
