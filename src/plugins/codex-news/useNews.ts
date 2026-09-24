/** 可见页面轮询、缓存和已读状态；请求合并、卸载失效及串行落盘由此持有。 */
import { computed, onUnmounted, ref, watch } from 'vue'
import { useToolLifecycle } from '@/core/lifecycle'
import type { NewsCache, NewsEvent, NewsSnapshot } from './contracts'
import { ipc } from './ipc'
import { changedEvents, parseFeeds, revision, validateCache } from './news'

// 同一工具关闭再打开时，读取必须等上一实例的写入完成，避免旧已读记录倒灌。
let cacheWrites = Promise.resolve()

export function useNews() {
  const { visibility, scope } = useToolLifecycle('codex-news', { dispose: stop })
  const snapshot = ref<NewsSnapshot | null>(null),
    pending = ref<NewsSnapshot | null>(null)
  const read = ref<Record<string, string>>({}),
    auto = ref(true)
  const checkedAt = ref(''),
    error = ref(''),
    saveError = ref(''),
    loading = ref(false),
    ready = ref(false)
  const now = ref(Date.now())
  let alive = true,
    allowSave = true,
    timer: ReturnType<typeof setTimeout> | undefined
  let writes = Promise.resolve(),
    request: Promise<void> | undefined
  let applyOnFinish = false
  const visible = computed(
    () => visibility.value.active && !visibility.value.covered && !visibility.value.hidden
  )
  const pendingCount = computed(() =>
    snapshot.value && pending.value ? changedEvents(snapshot.value, pending.value).length : 0
  )
  const stale = computed(
    () =>
      snapshot.value && (snapshot.value.stale || now.value > Date.parse(snapshot.value.freshUntil))
  )
  const unread = (item: NewsEvent) => read.value[item.id] !== revision(item)

  function persist() {
    if (!allowSave) return Promise.resolve()
    const current = pending.value ?? snapshot.value
    const ids = new Set(current?.events.map((item) => item.id))
    const value: NewsCache = {
      version: 1,
      snapshot: current,
      auto: auto.value,
      checkedAt: checkedAt.value,
      read: Object.fromEntries(Object.entries(read.value).filter(([id]) => ids.has(id))),
    }
    writes = cacheWrites
      .then(() => ipc.save(value))
      .then(() => {
        if (alive) saveError.value = ''
      })
      .catch((e) => {
        if (alive) saveError.value = `保存失败：${String(e)}`
      })
    cacheWrites = writes
    return writes
  }
  function applyPending() {
    if (pending.value) snapshot.value = pending.value
    pending.value = null
  }
  function markRead(item: NewsEvent) {
    read.value = { ...read.value, [item.id]: revision(item) }
    void persist()
  }
  function markAllRead() {
    read.value = {
      ...read.value,
      ...Object.fromEntries(
        (snapshot.value?.events ?? []).map((item) => [item.id, revision(item)])
      ),
    }
    void persist()
  }
  function schedule() {
    clearTimeout(timer)
    timer = undefined
    if (alive && ready.value && visible.value && auto.value && !loading.value)
      timer = setTimeout(() => {
        void refresh(false)
      }, 5 * 60_000)
  }
  function refresh(apply = true): Promise<void> {
    if (!alive || !ready.value) return Promise.resolve()
    applyOnFinish ||= apply
    if (request) return request
    loading.value = true
    now.value = Date.now()
    error.value = ''
    clearTimeout(timer)
    request = (async () => {
      try {
        const next = parseFeeds(await ipc.fetch())
        if (!alive) return
        const first = snapshot.value === null || snapshot.value.source !== next.source
        if (first)
          read.value = Object.fromEntries(next.events.map((item) => [item.id, revision(item)]))
        if (first || applyOnFinish) {
          snapshot.value = next
          pending.value = null
        } else if (changedEvents(snapshot.value!, next).length) {
          // 全量快照中消失的事件已撤回，即使新增内容待用户查看也立即移除。
          const ids = new Set(next.events.map((item) => item.id))
          snapshot.value = {
            ...snapshot.value!,
            events: snapshot.value!.events.filter((item) => ids.has(item.id)),
          }
          pending.value = next
        } else {
          // 仅更新时间变化时保留当前行次序，避免自动检查打断阅读。
          const events = new Map(next.events.map((item) => [item.id, item]))
          snapshot.value = {
            ...next,
            events: snapshot.value!.events.flatMap((item) => events.get(item.id) ?? []),
          }
          pending.value = null
        }
        checkedAt.value = new Date().toISOString()
        now.value = Date.now()
        await persist()
      } catch (e) {
        if (alive) error.value = `更新失败：${String(e)}`
      } finally {
        request = undefined
        applyOnFinish = false
        if (alive) {
          loading.value = false
          schedule()
        }
      }
    })()
    return request
  }
  async function initialize() {
    try {
      await cacheWrites
      if (!alive) return
      const raw = await ipc.load()
      if (!alive) return
      if (raw !== null) {
        const cache = validateCache(raw)
        snapshot.value = cache.snapshot
        read.value = cache.read
        auto.value = cache.auto
        checkedAt.value = cache.checkedAt
      }
    } catch (e) {
      if (!alive) return
      allowSave = false
      saveError.value = `缓存读取失败，暂不覆盖本地记录：${String(e)}`
    }
    if (!alive) return
    ready.value = true
    if (visible.value) void refresh(false)
  }
  function setAuto(value: boolean) {
    auto.value = value
    schedule()
    void persist()
  }
  watch(visible, (value) => {
    now.value = Date.now()
    if (value && ready.value) void refresh(false)
    else schedule()
  })
  function stop() {
    alive = false
    clearTimeout(timer)
    return writes
  }
  scope.interval(() => {
    if (visible.value) now.value = Date.now()
  }, 60_000)
  onUnmounted(() => {
    void stop()
  })
  void initialize()
  return {
    snapshot,
    pendingCount,
    auto,
    checkedAt,
    error,
    saveError,
    loading,
    ready,
    stale,
    unread,
    refresh,
    applyPending,
    markRead,
    markAllRead,
    setAuto,
  }
}
