/**
 * SSH 服务器分组（纯逻辑 + localStorage 持久化；折叠状态不持久化，每次打开工具默认全部折叠）
 * 分组只有一层；「未分组」是固定沉底的虚拟组（无实体记录）。
 */
import { ref, watch } from 'vue'

export interface ServerGroup {
  /** 唯一 id（group-<时间戳>） */
  id: string
  /** 分组名称 */
  name: string
  /** 排序权重（创建时自增，v1 按创建顺序排列） */
  order: number
}

const GROUPS_KEY = 'ssh.groups.v1'

/** 读取分组列表（数据损坏回退空列表） */
function loadGroups(): ServerGroup[] {
  try {
    const raw = localStorage.getItem(GROUPS_KEY)
    if (raw) return (JSON.parse(raw) as ServerGroup[]).sort((a, b) => a.order - b.order)
  } catch {
    /* 数据损坏时回退空列表 */
  }
  return []
}

export function useServerGroups() {
  const groups = ref<ServerGroup[]>(loadGroups())
  /** 展开的分组 id 集合（默认空 = 全部折叠；不持久化，重开工具回到全折叠） */
  const expandedIds = ref<Set<string>>(new Set())

  watch(groups, (list) => localStorage.setItem(GROUPS_KEY, JSON.stringify(list)), { deep: true })

  /** 新建分组（返回新分组；调用方 toast） */
  function createGroup(name: string): ServerGroup {
    const group: ServerGroup = {
      id: `group-${Date.now()}`,
      name: name.trim(),
      order: Math.max(0, ...groups.value.map((g) => g.order)) + 1,
    }
    groups.value = [...groups.value, group]
    return group
  }

  function renameGroup(id: string, name: string) {
    groups.value = groups.value.map((g) => (g.id === id ? { ...g, name: name.trim() } : g))
  }

  /** 删除分组（调用方负责把组内 profile 的 groupId 清掉） */
  function deleteGroup(id: string) {
    groups.value = groups.value.filter((g) => g.id !== id)
    const next = new Set(expandedIds.value)
    next.delete(id)
    expandedIds.value = next
  }

  function toggleGroup(id: string) {
    const next = new Set(expandedIds.value)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    expandedIds.value = next
  }

  return { groups, expandedIds, createGroup, renameGroup, deleteGroup, toggleGroup }
}
