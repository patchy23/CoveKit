import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, nextTick } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { publishToolVisibility, resetToolVisibilityForTest } from '@/core/lifecycle'
import { useNews } from './useNews'
import { parseFeeds, revision } from './news'
import { feeds, rawEvent } from './testFixtures'
const mock = vi.hoisted(() => ({ fetch: vi.fn(), load: vi.fn(), save: vi.fn() }))
vi.mock('./ipc', () => ({ ipc: mock }))
enableAutoUnmount(afterEach)
beforeEach(() => {
  vi.useFakeTimers()
  vi.resetAllMocks()
  resetToolVisibilityForTest()
  publishToolVisibility('codex-news', { active: true, covered: false, hidden: false })
  mock.load.mockResolvedValue(null)
  mock.fetch.mockResolvedValue(feeds())
  mock.save.mockResolvedValue(undefined)
})
afterEach(() => {
  vi.useRealTimers()
  resetToolVisibilityForTest()
})
function setup() {
  let api!: ReturnType<typeof useNews>
  const wrapper = mount(
    defineComponent({
      setup() {
        api = useNews()
        return () => h('div')
      },
    })
  )
  return { api, wrapper }
}
it('首次历史已读，后台检查暂存新增消息，用户应用后可标为已读', async () => {
  const { api } = setup()
  await flushPromises()
  expect(api.unread(api.snapshot.value!.events[0]!)).toBe(false)
  mock.fetch.mockResolvedValue(
    feeds([rawEvent('new', 'landed', '2026-09-18T12:00:00Z'), rawEvent()])
  )
  await api.refresh(false)
  expect(api.pendingCount.value).toBe(1)
  expect(api.snapshot.value!.events).toHaveLength(1)
  api.applyPending()
  const item = api.snapshot.value!.events[0]!
  expect(api.unread(item)).toBe(true)
  api.markRead(item)
  await flushPromises()
  expect(mock.save.mock.lastCall![0].read.new).toBe(revision(item))
})
it('可见时轮询，关闭自动更新或隐藏页面停止定时，返回页面补查', async () => {
  const { api, wrapper } = setup()
  await flushPromises()
  await vi.advanceTimersByTimeAsync(300000)
  expect(mock.fetch).toHaveBeenCalledTimes(2)
  publishToolVisibility('codex-news', { active: false })
  await nextTick()
  await vi.advanceTimersByTimeAsync(600000)
  expect(mock.fetch).toHaveBeenCalledTimes(2)
  publishToolVisibility('codex-news', { active: true })
  await flushPromises()
  expect(mock.fetch).toHaveBeenCalledTimes(3)
  api.setAuto(false)
  await flushPromises()
  await vi.advanceTimersByTimeAsync(600000)
  expect(mock.fetch).toHaveBeenCalledTimes(3)
  wrapper.unmount()
  expect(vi.getTimerCount()).toBe(0)
})
it('合并并发刷新，卸载后迟到响应不写入缓存', async () => {
  let finish!: (value: ReturnType<typeof feeds>) => void
  mock.fetch.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve
      })
  )
  const { api, wrapper } = setup()
  await flushPromises()
  void api.refresh()
  void api.refresh()
  expect(mock.fetch).toHaveBeenCalledTimes(1)
  wrapper.unmount()
  finish(feeds())
  await flushPromises()
  expect(mock.save).not.toHaveBeenCalled()
})

it('保存失败可见且后续写入可恢复，关闭再打开前先等待旧写入', async () => {
  mock.save.mockRejectedValueOnce(new Error('disk full'))
  const first = setup()
  await flushPromises()
  expect(first.api.saveError.value).toContain('disk full')
  let complete!: () => void
  mock.save.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        complete = resolve
      })
  )
  first.api.setAuto(false)
  await flushPromises()
  first.wrapper.unmount()
  mock.load.mockClear()
  const second = setup()
  await flushPromises()
  expect(mock.load).not.toHaveBeenCalled()
  complete()
  await flushPromises()
  expect(mock.load).toHaveBeenCalledTimes(1)
  expect(second.api.saveError.value).toBe('')
})

it('源只有时间元数据变化时保留行顺序，断网后仍能显示过期提示', async () => {
  vi.setSystemTime(new Date('2026-09-18T00:00:00Z'))
  mock.fetch.mockResolvedValue(
    feeds([rawEvent('old'), rawEvent('new', 'landed', '2026-09-18T00:00:00Z')])
  )
  const { api } = setup()
  await flushPromises()
  expect(api.stale.value).toBe(false)
  mock.fetch.mockResolvedValue(
    feeds([
      { ...rawEvent('old'), updated_at: '2026-09-19T00:00:00Z' },
      rawEvent('new', 'landed', '2026-09-18T00:00:00Z'),
    ])
  )
  await api.refresh(false)
  expect(api.pendingCount.value).toBe(0)
  expect(api.snapshot.value!.events.map((item) => item.id)).toEqual(['new', 'old'])
  api.setAuto(false)
  await vi.advanceTimersByTimeAsync(31 * 60_000)
  expect(api.stale.value).toBe(true)
})
it('读取已读缓存，失败保留列表，损坏缓存不被覆盖', async () => {
  const snapshot = parseFeeds(feeds())
  mock.load.mockResolvedValue({
    version: 1,
    snapshot,
    read: { one: revision(snapshot.events[0]!) },
    auto: false,
    checkedAt: '',
  })
  mock.fetch.mockRejectedValue(new Error('offline'))
  const first = setup()
  await flushPromises()
  expect(first.api.snapshot.value).toEqual(snapshot)
  expect(first.api.error.value).toContain('offline')
  expect(first.api.unread(snapshot.events[0]!)).toBe(false)
  first.wrapper.unmount()
  mock.load.mockResolvedValue({ version: 9 })
  mock.fetch.mockResolvedValue(feeds())
  mock.save.mockClear()
  const second = setup()
  await flushPromises()
  expect(second.api.saveError.value).toContain('缓存读取失败')
  expect(mock.save).not.toHaveBeenCalled()
})
