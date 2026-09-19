import type { UiTreeItem } from './collection/types'

export interface TreeSelectOption {
  value: string
  label: string
  disabled?: boolean
  children?: TreeSelectOption[]
}

/** 搜索保留命中节点的祖先；路径用于回显与搜索，节点身份由 value 提供。 */
export function treeSelectRows(options: TreeSelectOption[], expanded: Set<string>, query = '') {
  const rows: UiTreeItem[] = []
  const labels = new Map<string, string>()
  const needle = query.trim().toLocaleLowerCase()
  function visit(nodes: TreeSelectOption[], parents: string[], depth: number): UiTreeItem[] {
    return nodes.flatMap((node) => {
      const path = [...parents, node.label]
      labels.set(node.value, path.join(' / '))
      const children = visit(node.children ?? [], path, depth + 1)
      if (needle && !path.join(' / ').toLocaleLowerCase().includes(needle) && !children.length)
        return []
      const open = !!needle || expanded.has(node.value)
      return [
        {
          id: node.value,
          label: node.label,
          depth,
          disabled: node.disabled,
          expandable: !!node.children?.length,
          expanded: open,
        },
        ...(open ? children : []),
      ]
    })
  }
  rows.push(...visit(options, [], 0))
  return { rows, labels }
}
