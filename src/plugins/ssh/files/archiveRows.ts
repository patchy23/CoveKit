import type { ArchiveEntry } from '../contracts'

const compare = (a: ArchiveEntry, b: ArchiveEntry) =>
  Number(b.isDir) - Number(a.isDir) || a.path.localeCompare(b.path)

/** 输入由预览 owner 只追加、重读时整体替换；只缓存当前目录或搜索，不复制整棵归档树。 */
export function createArchiveRows() {
  let source: readonly ArchiveEntry[] | undefined
  let mode = ''
  let processed = 0
  let children = new Map<string, ArchiveEntry>()
  let rows: ArchiveEntry[] = []
  return (entries: readonly ArchiveEntry[], directory: string, query: string): ArchiveEntry[] => {
    const keyword = query.toLowerCase()
    const nextMode = keyword ? `search:${keyword}` : `directory:${directory}`
    if (source !== entries || mode !== nextMode || processed > entries.length) {
      source = entries
      mode = nextMode
      processed = 0
      children = new Map()
      rows = []
    }
    if (keyword) {
      const added: ArchiveEntry[] = []
      for (; processed < entries.length; processed++) {
        const entry = entries[processed]
        if (entry.path.toLowerCase().includes(keyword)) added.push(entry)
      }
      if (added.length) rows = rows.concat(added)
      return rows
    }
    const added = new Map<string, ArchiveEntry>()
    let replaced = false
    for (; processed < entries.length; processed++) {
      const entry = entries[processed]
      if (!entry.path.startsWith(directory)) continue
      const relative = entry.path.slice(directory.length)
      if (!relative) continue
      const separator = relative.indexOf('/')
      const path = directory + (separator < 0 ? relative : relative.slice(0, separator))
      const previous = children.get(path)
      if (separator >= 0 && previous) continue
      const child = separator < 0 ? entry : { path, isDir: true, size: null, modifiedAt: 0 }
      if (previous === child) continue
      if (previous && !added.has(path)) replaced = true
      children.set(path, child)
      added.set(path, child)
    }
    if (!added.size) return rows
    // 显式目录替换合成目录等少见情况保留 Map 插入次序和稳定排序语义。
    if (replaced) {
      rows = [...children.values()].sort(compare)
      return rows
    }
    const incoming = [...added.values()].sort(compare)
    if (!rows.length) {
      rows = incoming
      return rows
    }
    const merged: ArchiveEntry[] = []
    let left = 0,
      right = 0
    while (left < rows.length && right < incoming.length) {
      if (compare(rows[left], incoming[right]) <= 0) merged.push(rows[left++])
      else merged.push(incoming[right++])
    }
    while (left < rows.length) merged.push(rows[left++])
    while (right < incoming.length) merged.push(incoming[right++])
    rows = merged
    return rows
  }
}
