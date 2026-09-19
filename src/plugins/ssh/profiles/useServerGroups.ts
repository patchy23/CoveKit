/**
 * SSH 服务器分组（后端 ssh.db 持久化；折叠状态不持久化，每次打开工具默认全部折叠）
 * 分组只有一层；「未分组」是固定沉底的虚拟组（无实体记录）。
 */
import { ref } from 'vue'
import type { SshGroup } from '../contracts'
import { ipc } from '../ipc'

/** 兼容别名：分组模型已迁移到 contracts.SshGroup */
export type ServerGroup = SshGroup

export function useServerGroups() {
  const groups = ref<SshGroup[]>([])
  /** 展开的分组 id 集合（默认空 = 全部折叠；不持久化，重开工具回到全折叠） */
  const expandedIds = ref<Set<string>>(new Set())

  /** 从后端加载分组（失败回退空列表，不阻断 SSH 工具打开） */
  async function load(): Promise<void> {
    try {
      groups.value = await ipc.sshGroupList()
    } catch {
      groups.value = []
    }
  }

  /** 新建分组（返回新分组；调用方 toast） */
  async function createGroup(name: string): Promise<SshGroup> {
    const group: SshGroup = {
      id: `group-${crypto.randomUUID()}`,
      name: name.trim(),
      sortOrder: Math.max(0, ...groups.value.map((g) => g.sortOrder)) + 1,
    }
    await ipc.sshGroupSave(group)
    groups.value = [...groups.value, group]
    return group
  }

  async function renameGroup(id: string, name: string) {
    const group = groups.value.find((g) => g.id === id)
    if (!group) return
    const renamed = { ...group, name: name.trim() }
    await ipc.sshGroupSave(renamed)
    groups.value = groups.value.map((g) => (g.id === id ? renamed : g))
  }

  /** 删除分组（后端负责把组内 profile 的 groupId 清掉） */
  async function deleteGroup(id: string) {
    await ipc.sshGroupDelete(id)
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

  return { groups, expandedIds, load, createGroup, renameGroup, deleteGroup, toggleGroup }
}

export { UNGROUPED_DROP_KEY } from './serverTree'
