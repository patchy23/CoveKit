/** 公共集合只描述展示与移动意图，不读写业务数据。 */
export interface UiListItem {
  id: string
  label: string
  /** 默认仅分支显示文件夹，叶子无默认图标；false 同时隐藏业务图标插槽，层级缩进不变。 */
  showIcon?: boolean
  description?: string
  kind?: string
  badge?: string | number
  muted?: boolean
  disabled?: boolean
  draggable?: boolean
}

/** items 为按深度优先排列的可见节点；展开状态由调用方持有。 */
export interface UiTreeItem extends UiListItem {
  depth: number
  expanded?: boolean
  expandable?: boolean
  loading?: boolean
}

export interface UiCollectionMove {
  id: string
  /** null 表示根层末尾。 */
  targetId: string | null
  position: 'before' | 'after' | 'inside'
}

export type UiDropGuard = (move: UiCollectionMove) => boolean

export function parentOf(items: UiTreeItem[], id: string): UiTreeItem | undefined {
  const index = items.findIndex((item) => item.id === id)
  const item = items[index]
  if (!item) return
  for (let i = index - 1; i >= 0; i--) if (items[i].depth < item.depth) return items[i]
}

/** 即使业务没有提供 guard，也拒绝自身、后代和不可操作节点。 */
export function validMove(items: UiTreeItem[], move: UiCollectionMove, tree: boolean): boolean {
  const source = items.find((item) => item.id === move.id)
  if (!source || source.disabled || source.draggable === false) return false
  if (move.targetId === null) return move.position === 'inside'
  const target = items.find((item) => item.id === move.targetId)
  if (!target || target.disabled || target.id === source.id) return false
  if (move.position === 'inside' && (!tree || !target.expandable)) return false
  let ancestor: UiTreeItem | undefined = target
  while (ancestor) {
    if (ancestor.id === source.id) return false
    ancestor = parentOf(items, ancestor.id)
  }
  return true
}
