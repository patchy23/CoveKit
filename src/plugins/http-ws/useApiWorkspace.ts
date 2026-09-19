/** 接口库与多页签草稿；保存捕获目标和快照，异步完成不覆盖后续编辑。 */
import { computed, reactive, ref, shallowReactive } from 'vue'
import { ipc } from './ipc'
import type { UiCollectionMove } from '@/core/ui'
import { apiTreeDestination } from './apiTree'
import { fingerprint, fromRecord, newDraft, toRecord, type RequestDraft } from './requestDraft'
import { useRequestSession, type RequestSession } from './useRequestSession'
import type { ApiKind, ApiRecord } from './contracts'

export interface ApiTab {
  key: string
  recordId: number | null
  name: string
  groupName: string
  draft: RequestDraft
  saved: string
  saving: boolean
  session: RequestSession
}
export function useApiWorkspace(report: (message: string) => void, visible: () => boolean) {
  const apis = ref<ApiRecord[]>([])
  const groups = ref<string[]>([])
  // 身份独立于路径，移动整棵目录时选中和折叠仍引用同一节点。
  const groupIdentities = ref<Record<string, string>>({})
  const moving = ref(new Set<number>())
  const groupMoving = ref(false)
  let pendingWrites = 0
  async function libraryWrite<T>(action: () => Promise<T>) {
    if (groupMoving.value) throw new Error('分组正在移动，请稍后重试')
    pendingWrites++
    try {
      return await action()
    } finally {
      pendingWrites--
    }
  }
  const tabs = shallowReactive<ApiTab[]>([])
  const activeKey = ref('')
  const loading = ref(false)
  const loadError = ref('')
  const current = computed(() => tabs.find((t) => t.key === activeKey.value))
  let loadEpoch = 0
  let disposed = false
  function dirty(tab: ApiTab) {
    return tab.recordId === null || fingerprint(tab.draft) !== tab.saved
  }
  async function load() {
    const token = ++loadEpoch
    loading.value = true
    try {
      const [list, paths] = await Promise.all([ipc.apiList(), ipc.apiGroupList()])
      if (token === loadEpoch && !disposed) {
        apis.value = list
        groups.value = paths
        groupIdentities.value = Object.fromEntries(
          paths.map((path) => [path, groupIdentities.value[path] ?? crypto.randomUUID()])
        )
        loadError.value = ''
      }
    } catch (error) {
      if (token === loadEpoch) loadError.value = `读取接口失败：${String(error)}`
    } finally {
      if (token === loadEpoch) loading.value = false
    }
  }
  function add(draft: RequestDraft, record?: ApiRecord) {
    const key = crypto.randomUUID()
    const tab = reactive({
      key,
      recordId: record?.id ?? null,
      name: record?.name || `未命名 ${draft.type.toUpperCase()}`,
      groupName: record?.groupName || '',
      draft,
      saved: fingerprint(draft),
      saving: false,
    })
    const session = useRequestSession(
      () => tab.draft,
      report,
      () => visible() && activeKey.value === key
    )
    const result: ApiTab = Object.assign(tab, { session })
    tabs.push(result)
    activeKey.value = key
    return result
  }
  function create(kind: ApiKind, groupName = '') {
    const tab = add(newDraft(kind))
    tab.groupName = groupName
    return tab
  }
  async function createGroup(name: string, parent: string) {
    const path = await ipc.apiGroupCreate(name, parent)
    await load()
    return path
  }
  function open(record: ApiRecord) {
    const existing = tabs.find((t) => t.recordId === record.id)
    if (existing) {
      activeKey.value = existing.key
      return existing
    }
    return add(fromRecord(record), record)
  }
  async function save(tab: ApiTab, name: string, groupName: string, asCopy = false) {
    if (groupMoving.value) throw new Error('分组正在移动，请稍后保存')
    if (tab.recordId !== null && moving.value.has(tab.recordId))
      throw new Error('接口正在移动，请稍后保存')
    if (tab.saving) return false
    if (!name.trim()) throw new Error('请输入接口名称')
    const snapshot = fingerprint(tab.draft)
    const payload = toRecord(tab.draft, name, groupName, asCopy ? 0 : (tab.recordId ?? 0))
    tab.saving = true
    try {
      const id = await ipc.apiSave(payload)
      if (tabs.includes(tab)) {
        tab.recordId = id
        tab.name = payload.name
        tab.groupName = payload.groupName
        tab.saved = snapshot
      }
      await load()
      report(
        '接口已保存' +
          (tab.draft.auth.secret && !tab.draft.auth.credentialId
            ? '；临时认证仅保留在当前页签'
            : '')
      )
      return true
    } finally {
      tab.saving = false
    }
  }
  /** 只提交元数据，同一接口的保存、重命名和删除互斥。 */
  async function rename(record: ApiRecord, name: string, groupName: string) {
    if (moving.value.has(record.id)) throw new Error('接口正在移动，请稍后修改')
    if (tabs.some((tab) => tab.recordId === record.id && tab.saving))
      throw new Error('接口正在保存，请稍后修改')
    if (!name.trim()) throw new Error('请输入接口名称')
    moving.value.add(record.id)
    try {
      await ipc.apiRename(record.id, name.trim(), groupName.trim())
      const tab = tabs.find((t) => t.recordId === record.id)
      if (tab) {
        tab.name = name.trim()
        tab.groupName = groupName.trim()
      }
      await load()
    } finally {
      moving.value.delete(record.id)
    }
  }
  async function remove(record: ApiRecord) {
    if (moving.value.has(record.id)) throw new Error('接口正在移动，请稍后删除')
    if (tabs.some((tab) => tab.recordId === record.id && tab.saving))
      throw new Error('接口正在保存，请稍后删除')
    moving.value.add(record.id)
    try {
      await ipc.apiDelete(record.id)
      for (const tab of tabs.filter((t) => t.recordId === record.id)) {
        tab.recordId = null
        tab.saved = ''
      }
      await load()
    } finally {
      moving.value.delete(record.id)
    }
  }
  async function close(tab: ApiTab) {
    if (tab.saving) throw new Error('接口正在保存，请稍后关闭')
    await tab.session.stop()
    await tab.session.dispose()
    const index = tabs.indexOf(tab)
    if (index < 0) return
    tabs.splice(index, 1)
    if (activeKey.value === tab.key) activeKey.value = (tabs[index] || tabs[index - 1])?.key || ''
  }
  async function move(id: number, groupName: string) {
    if (groupMoving.value) throw new Error('分组正在移动，请稍后重试')
    const record = apis.value.find((api) => api.id === id)
    if (!record || record.groupName === groupName) return
    if (moving.value.has(id) || tabs.some((tab) => tab.recordId === id && tab.saving))
      throw new Error('接口正在保存或移动，请稍后重试')
    moving.value.add(id)
    try {
      await ipc.apiMoveGroup(id, groupName)
      if (!disposed) {
        record.groupName = groupName
        for (const tab of tabs.filter((tab) => tab.recordId === id)) tab.groupName = groupName
      }
      await load()
      report(`已移动到 ${groupName || '未分组'}`)
    } finally {
      moving.value.delete(id)
    }
  }
  async function moveTree(move: UiCollectionMove) {
    if (groupMoving.value || pendingWrites || moving.value.size || tabs.some((tab) => tab.saving))
      throw new Error('接口库正在保存或移动，请稍后重试')
    const payload = apiTreeDestination(move, apis.value, groupIdentities.value)
    if (!payload) throw new Error('不能放置在此处')
    groupMoving.value = true
    try {
      const destination = await ipc.apiTreeMove(payload)
      if (disposed) return
      if (payload.kind === 'group') {
        const remap = (path: string) =>
          path === payload.id || path.startsWith(`${payload.id}/`)
            ? destination + path.slice(payload.id.length)
            : path
        for (const tab of tabs) tab.groupName = remap(tab.groupName)
        groupIdentities.value = Object.fromEntries(
          Object.entries(groupIdentities.value).map(([path, key]) => [remap(path), key])
        )
      } else {
        for (const tab of tabs)
          if (String(tab.recordId) === payload.id) tab.groupName = payload.parent
      }
      await load()
      report('位置已保存')
    } finally {
      groupMoving.value = false
    }
  }
  async function dispose() {
    disposed = true
    loadEpoch++
    const results = await Promise.allSettled(tabs.map((t) => t.session.dispose()))
    const failures = results.filter((r): r is PromiseRejectedResult => r.status === 'rejected')
    if (failures.length) throw new Error(failures.map((r) => String(r.reason)).join('；'))
  }
  async function moveGroup(path: string, parent: string) {
    if (groupMoving.value || pendingWrites || moving.value.size || tabs.some((tab) => tab.saving))
      throw new Error('接口库正在保存或移动，请稍后重试')
    if (!path || parent === path || parent.startsWith(`${path}/`))
      throw new Error('不能将分组移入自身或子分组')
    groupMoving.value = true
    try {
      const destination = await ipc.apiGroupMove(path, parent)
      const movedPath = (value: string) =>
        value === path || value.startsWith(`${path}/`)
          ? destination + value.slice(path.length)
          : value
      if (!disposed) {
        for (const api of apis.value) api.groupName = movedPath(api.groupName)
        // 包括尚未保存的新接口；移动不改变草稿快照或连接对象。
        for (const tab of tabs) tab.groupName = movedPath(tab.groupName)
        groups.value = groups.value.map(movedPath)
        groupIdentities.value = Object.fromEntries(
          Object.entries(groupIdentities.value).map(([path, key]) => [movedPath(path), key])
        )
      }
      await load()
      report(`分组已移至 ${parent || '根目录'}`)
    } finally {
      groupMoving.value = false
    }
  }
  return {
    apis,
    groups,
    groupIdentities,
    moving,
    groupMoving,
    tabs,
    activeKey,
    current,
    loading,
    loadError,
    dirty,
    load,
    create,
    createGroup: (name: string, parent: string) => libraryWrite(() => createGroup(name, parent)),
    open,
    save,
    rename: (record: ApiRecord, name: string, groupName: string) =>
      libraryWrite(() => rename(record, name, groupName)),
    remove: (record: ApiRecord) => libraryWrite(() => remove(record)),
    move,
    moveGroup,
    moveTree,
    close,
    dispose,
  }
}
