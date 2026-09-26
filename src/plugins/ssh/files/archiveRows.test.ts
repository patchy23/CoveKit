import { computed, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import type { ArchiveEntry } from '../contracts'
import { createArchiveRows } from './archiveRows'

afterEach(() => vi.restoreAllMocks())
const entry = (path: string, isDir = false, size: number | null = 10): ArchiveEntry => ({
  path,
  isDir,
  size,
  modifiedAt: 0,
})

function original(entries: ArchiveEntry[], directory: string, query: string) {
  if (query)
    return entries.filter((value) => value.path.toLowerCase().includes(query.toLowerCase()))
  const children = new Map<string, ArchiveEntry>()
  for (const value of entries) {
    if (!value.path.startsWith(directory)) continue
    const relative = value.path.slice(directory.length)
    if (!relative) continue
    const path = directory + relative.split('/')[0]
    if (relative.includes('/')) {
      if (!children.has(path)) children.set(path, entry(path, true, null))
    } else children.set(path, value)
  }
  return [...children.values()].sort(
    (a, b) => Number(b.isDir) - Number(a.isDir) || a.path.localeCompare(b.path)
  )
}

it('多批条目、显式目录替换、重名和嵌套路径保持原目录与搜索结果', () => {
  const source: ArchiveEntry[] = []
  const root = createArchiveRows(),
    nested = createArchiveRows(),
    search = createArchiveRows()
  const batches = [
    [entry('z'), entry('dir/child'), entry('a')],
    [entry('dir', true), entry('dir/deep/file'), entry('B'), entry('b')],
    [entry('a', false, 100), entry('other/file'), entry('dir/child', false, 55)],
    [entry('dir/Ä'), entry('dir/A\u0308'), entry('B', true)],
  ]
  for (const batch of batches) {
    source.push(...batch)
    expect(root(source, '', '')).toEqual(original(source, '', ''))
    expect(nested(source, 'dir/', '')).toEqual(original(source, 'dir/', ''))
    expect(search(source, '', 'DIR')).toEqual(original(source, '', 'DIR'))
  }
})

it('追加只读取新条目，目录无变化时复用已排序结果，切换与重读完整重建', () => {
  const source = [entry('folder/a'), entry('root.txt')]
  const read = vi.fn(() => 'folder/a')
  Object.defineProperty(source[0], 'path', { get: read })
  const project = createArchiveRows()
  const initial = project(source, '', '')
  read.mockClear()
  source.push(entry('folder/b'))
  expect(project(source, '', '')).toBe(initial)
  expect(read).not.toHaveBeenCalled()
  source.push(entry('new.txt'))
  expect(project(source, '', '').map((value) => value.path)).toEqual([
    'folder',
    'new.txt',
    'root.txt',
  ])
  expect(read).not.toHaveBeenCalled()
  expect(project(source, 'folder/', '').map((value) => value.path)).toEqual([
    'folder/a',
    'folder/b',
  ])
  expect(read).toHaveBeenCalled()
  expect(project([entry('fresh')], '', '').map((value) => value.path)).toEqual(['fresh'])
})

it('响应式追加会更新分页消费，大小写等价查询不重复派生，替换和缩短不会残留旧结果', () => {
  const source = ref([entry('a'), entry('b')])
  const query = ref('')
  const project = createArchiveRows()
  const rows = computed(() => project(source.value, '', query.value))
  const page = computed(() => rows.value.slice(0, 200))
  expect(page.value.map((value) => value.path)).toEqual(['a', 'b'])
  source.value.push(entry('c'))
  expect(page.value.map((value) => value.path)).toEqual(['a', 'b', 'c'])
  query.value = 'B'
  const searched = rows.value
  query.value = 'b'
  expect(rows.value).toBe(searched)
  source.value = [entry('new-b')]
  expect(page.value.map((value) => value.path)).toEqual(['new-b'])
  source.value.length = 0
  expect(page.value).toEqual([])
})
