/**
 * SSH 服务器分组（纯逻辑 + localStorage 持久化；折叠状态不持久化，每次打开工具默认全部折叠）
 * 分组只有一层；「未分组」是固定沉底的虚拟组（无实体记录）。
 */
import { ref, watch } from 'vue'
import type { ServerProfile } from './contracts'

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

/* ── 拖拽入组（pointer 事件自实现；HTML5 DnD 与 Tauri dragDropEnabled 的 OLE 拖放冲突，实测不可用） ── */

/** 拖动超过该位移才算拖拽（避免吞掉单击/双击） */
const DRAG_THRESHOLD = 6

/** 未分组投放目标的命中值（data-group-drop=""） */
export const UNGROUPED_DROP_KEY = '__ungrouped__'

export interface GroupDragState {
  profileId: string
  name: string
  startX: number
  startY: number
  x: number
  y: number
  /** 是否已越过阈值进入拖拽态 */
  active: boolean
}

/**
 * 连接行拖拽入组：行上挂 @pointerdown="onRowPointerDown($event, profile)"，
 * 分组行挂 data-group-drop（未分组为空串）；投放命中经回调上抛（groupId null = 未分组）。
 */
export function useGroupDrag(onMove: (profileId: string, groupId: string | null) => void) {
  const drag = ref<GroupDragState | null>(null)
  /** 当前命中的投放目标（高亮反馈） */
  const dragOverId = ref<string | null>(null)

  function onRowPointerDown(event: PointerEvent, profile: ServerProfile) {
    if (event.button !== 0) return
    drag.value = {
      profileId: profile.id,
      name: profile.name,
      startX: event.clientX,
      startY: event.clientY,
      x: event.clientX,
      y: event.clientY,
      active: false,
    }
    window.addEventListener('pointermove', onDragMove)
    window.addEventListener('pointerup', onDragEnd, { once: true })
    window.addEventListener('keydown', onDragKeydown)
  }

  /** 命中测试：指针下的分组行（data-group-drop 属性，空串 = 未分组） */
  function hitTest(x: number, y: number): string | null {
    const el = document.elementFromPoint(x, y)
    const target = el?.closest('[data-group-drop]') as HTMLElement | null
    if (!target) return null
    const raw = target.dataset.groupDrop
    return raw === '' ? UNGROUPED_DROP_KEY : (raw ?? null)
  }

  function onDragMove(event: PointerEvent) {
    const d = drag.value
    if (!d) return
    if (!d.active) {
      if (Math.hypot(event.clientX - d.startX, event.clientY - d.startY) < DRAG_THRESHOLD) return
      d.active = true
    }
    d.x = event.clientX
    d.y = event.clientY
    dragOverId.value = hitTest(event.clientX, event.clientY)
  }

  function cleanup() {
    drag.value = null
    dragOverId.value = null
    window.removeEventListener('pointermove', onDragMove)
    window.removeEventListener('keydown', onDragKeydown)
  }

  function onDragEnd(event: PointerEvent) {
    const d = drag.value
    cleanup()
    if (!d?.active) return
    const hit = hitTest(event.clientX, event.clientY)
    if (hit === null) return
    onMove(d.profileId, hit === UNGROUPED_DROP_KEY ? null : hit)
  }

  /** Esc 取消拖拽 */
  function onDragKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') cleanup()
  }

  return { drag, dragOverId, onRowPointerDown }
}
