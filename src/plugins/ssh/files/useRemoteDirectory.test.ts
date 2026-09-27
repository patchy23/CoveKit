import { effectScope, isProxy } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { useRemoteDirectory } from './useRemoteDirectory'
import type { RemoteFile } from '../contracts'

const mocks = vi.hoisted(() => ({ list: vi.fn(), toast: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: { sshFileList: mocks.list } }))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ toast: mocks.toast }) }))
afterEach(() => {
  vi.resetAllMocks()
  vi.useRealTimers()
})

function file(name: string, isDir = false, modifiedAt = 0): RemoteFile {
  return {
    name,
    path: '/' + name,
    isDir,
    modifiedAt,
    size: 1,
    permissions: '-',
    owner: '-',
    group: '-',
  }
}

it('当前目录长期保留，闲置十分钟自动过期且不主动刷新，关闭释放唯一计时器', async () => {
  vi.useFakeTimers()
  mocks.list.mockImplementation(async (_session, path) => ({ ok: true, files: [file(path)] }))
  const scope = effectScope()
  const directory = scope.run(() => useRemoteDirectory({ sessionId: () => 'session' }))!
  try {
    await directory.navigate('/a')
    const a = directory.files.value
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000)
    await directory.navigate('/a')
    expect(directory.files.value).toBe(a)
    expect(mocks.list).toHaveBeenCalledOnce()
    expect(vi.getTimerCount()).toBe(0)
    await directory.navigate('/b')
    expect(vi.getTimerCount()).toBe(1)
    await vi.advanceTimersByTimeAsync(10 * 60 * 1000)
    expect(mocks.list).toHaveBeenCalledTimes(2)
    expect(directory.files.value[0].name).toBe('/b')
    expect(vi.getTimerCount()).toBe(0)
    await directory.navigate('/a')
    expect(mocks.list).toHaveBeenCalledTimes(3)
    expect(directory.files.value).not.toBe(a)
    expect(vi.getTimerCount()).toBe(1)
  } finally {
    scope.stop()
  }
  expect(vi.getTimerCount()).toBe(0)
})

it('重新访问续期，离开后重新计时；强制刷新与切换连接仍读取新数据', async () => {
  vi.useFakeTimers()
  mocks.list.mockImplementation(async (_session, path) => ({ ok: true, files: [file(path)] }))
  const scope = effectScope()
  const directory = scope.run(() => useRemoteDirectory({ sessionId: () => 'session' }))!
  try {
    await directory.navigate('/a')
    const a = directory.files.value
    await directory.navigate('/b')
    await vi.advanceTimersByTimeAsync(9 * 60 * 1000)
    await directory.navigate('/a')
    expect(directory.files.value).toBe(a)
    await directory.navigate('/b')
    await vi.advanceTimersByTimeAsync(2 * 60 * 1000)
    await directory.navigate('/a')
    expect(directory.files.value).toBe(a)
    expect(mocks.list).toHaveBeenCalledTimes(2)
    await directory.refreshCurrent()
    expect(mocks.list).toHaveBeenCalledTimes(3)
    expect(directory.files.value).not.toBe(a)
    directory.reset(undefined)
    expect(vi.getTimerCount()).toBe(0)
    await directory.navigate('/a')
    expect(mocks.list).toHaveBeenCalledTimes(4)
  } finally {
    scope.stop()
  }
})

it('排序保持原区域、数值和目录优先语义，快照与行身份复用，刷新仍替换内容', async () => {
  const source = [
    file('file10'),
    file('file2'),
    file('B'),
    file('a'),
    file('中文'),
    file('folder', true),
  ]
  mocks.list.mockResolvedValue({ ok: true, files: source })
  const scope = effectScope()
  const directory = scope.run(() => useRemoteDirectory({ sessionId: () => 'session' }))!
  try {
    await directory.navigate('/a')
    expect(directory.files.value).toBe(source)
    expect(isProxy(directory.files.value[0])).toBe(false)
    const expected = [...source].sort((a, b) =>
      a.isDir !== b.isDir
        ? a.isDir
          ? -1
          : 1
        : a.name.localeCompare(b.name, 'zh-CN', { numeric: true, sensitivity: 'base' })
    )
    expect(directory.sortedFiles.value).toEqual(expected)
    directory.sortedFiles.value.forEach((entry, index) => expect(entry).toBe(expected[index]))
    directory.changeSort('name')
    expect(directory.sortedFiles.value).toEqual([expected[0], ...expected.slice(1).reverse()])
    await directory.navigate('/a')
    expect(mocks.list).toHaveBeenCalledOnce()
    const changed = [file('new', false, 100), file('old', false, 1), file('dir', true, 0)]
    mocks.list.mockResolvedValue({ ok: true, files: changed })
    await directory.refreshCurrent()
    directory.changeSort('modifiedAt')
    expect(directory.sortedFiles.value.map((entry) => entry.name)).toEqual(['dir', 'new', 'old'])
    expect(source.map((entry) => entry.name)).toEqual([
      'file10',
      'file2',
      'B',
      'a',
      '中文',
      'folder',
    ])
  } finally {
    scope.stop()
  }
})

it('关闭清空缓存和文件引用，迟到响应不能回填或再次导航', async () => {
  let resolve!: (value: unknown) => void
  mocks.list.mockResolvedValueOnce({ ok: true, files: [file('large')] }).mockReturnValueOnce(
    new Promise((done) => {
      resolve = done
    })
  )
  const scope = effectScope()
  const directory = scope.run(() => useRemoteDirectory({ sessionId: () => 'session' }))!
  await directory.navigate('/a')
  const pending = directory.navigate('/b')
  scope.stop()
  resolve({ ok: true, files: [file('late')] })
  await pending
  await directory.navigate('/a')
  expect(directory.files.value).toEqual([])
  expect(directory.directoryHistory.value).toEqual([])
  expect(mocks.list).toHaveBeenCalledTimes(2)
  expect(mocks.toast).not.toHaveBeenCalled()
})
