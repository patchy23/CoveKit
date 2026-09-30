import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'
import { UiButton, UiTooltip } from '@/core/ui'
import type { useDatabase } from './useDatabase'
import RedisKeyListFooter from './RedisKeyListFooter.vue'

enableAutoUnmount(afterEach)
afterEach(() => vi.unstubAllGlobals())

function state(overrides: Record<string, unknown> = {}) {
  return {
    connId: 'redis-test',
    database: 'db2',
    loaded: 3,
    total: 20 as number | null,
    hasMore: true,
    loading: false,
    fetchingAll: false,
    error: '',
    limitReached: false,
    limitReason: null as 'keys' | 'bytes' | null,
    autoLoadBudgetReached: false,
    automaticPaused: false,
    ...overrides,
  }
}

function createDb(initial = state()) {
  const current = ref(initial)
  const db = {
    redisAutoLoadEnabled: ref(false),
    redisKeyLoadState: vi.fn(() => current.value),
    loadMoreRedisKeys: vi.fn(async () => {}),
    fetchAllRedisKeys: vi.fn(async () => {}),
    stopRedisKeyLoad: vi.fn(),
    autoLoadMoreRedisKeys: vi.fn(async () => {}),
    pauseRedisKeyLoads: vi.fn(),
  } as unknown as ReturnType<typeof useDatabase>
  return { db, current }
}

class FakeIntersectionObserver {
  static instances: FakeIntersectionObserver[] = []
  readonly callback: IntersectionObserverCallback
  readonly rootMargin: string
  observed: Element | null = null
  disconnected = false

  constructor(callback: IntersectionObserverCallback, options?: IntersectionObserverInit) {
    this.callback = callback
    this.rootMargin = options?.rootMargin ?? ''
    FakeIntersectionObserver.instances.push(this)
  }

  observe(target: Element) {
    this.observed = target
  }

  unobserve() {}

  disconnect() {
    this.disconnected = true
  }

  takeRecords(): IntersectionObserverEntry[] {
    return []
  }

  trigger(isIntersecting: boolean) {
    if (!this.observed) return
    this.callback(
      [{ target: this.observed, isIntersecting } as IntersectionObserverEntry],
      this as unknown as IntersectionObserver
    )
  }
}

it('显示已加载数量和已知总数，包括空库零；未知总数不显示为零', async () => {
  const { db, current } = createDb(state({ loaded: 0, total: 0, hasMore: false }))
  const wrapper = mount(RedisKeyListFooter, {
    props: { db, nodeId: 'redis-test::redis:db2::keys::scan-more', active: true },
  })
  expect(wrapper.text()).toContain('0 / 0')
  expect(wrapper.findAll('button')).toHaveLength(0)

  current.value = state({ loaded: 1_234_567, total: null })
  await nextTick()
  expect(wrapper.text()).toContain('1,234,567')
  expect(wrapper.text()).not.toContain('/ 0')
  expect(wrapper.findAllComponents(UiTooltip).map((tooltip) => tooltip.props('content'))).toContain(
    '已加载 1,234,567，总数未知'
  )

  current.value = state({ loaded: 200, total: 20_000 })
  await nextTick()
  expect(wrapper.text()).toContain('200 / 20,000')
  expect(wrapper.findAllComponents(UiTooltip).map((tooltip) => tooltip.props('content'))).toContain(
    '已加载 200，总数 20,000'
  )
})

it('分批操作和限额说明保留紧凑入口', async () => {
  const { db, current } = createDb(state({ limitReached: true, limitReason: 'bytes' }))
  const wrapper = mount(RedisKeyListFooter, {
    props: { db, nodeId: 'redis-test::redis:db2::keys::scan-more', active: true },
  })
  expect(wrapper.text()).toContain('已达上限')
  expect(wrapper.findAll('button')).toHaveLength(0)
  expect(wrapper.findAllComponents(UiTooltip).map((tooltip) => tooltip.props('content'))).toContain(
    '键名数据达到加载上限（32 MiB）'
  )

  current.value = state()
  await nextTick()
  const buttons = wrapper.findAll('button')
  expect(buttons.map((button) => button.text())).toEqual(['更多', '全部'])
  expect(wrapper.findAllComponents(UiButton).map((button) => button.props('title'))).toEqual([
    '加载更多',
    '获取全部',
  ])
  await buttons[0]!.trigger('click')
  await buttons[1]!.trigger('click')
  expect(db.loadMoreRedisKeys).toHaveBeenCalledWith('redis-test::redis:db2::keys::scan-more')
  expect(db.fetchAllRedisKeys).toHaveBeenCalledWith('redis-test::redis:db2::keys::scan-more')
})

it('固定进度行显示目标库并始终提供停止全量读取', async () => {
  const { db } = createDb(state({ fetchingAll: true, loaded: 10, total: 200 }))
  const wrapper = mount(RedisKeyListFooter, {
    props: {
      db,
      nodeId: 'redis-test::redis:db2::keys::scan-more',
      active: true,
      mode: 'progress',
    },
  })
  expect(wrapper.text()).toContain('db2')
  expect(wrapper.text()).toContain('10 / 200')
  expect(wrapper.findAll('button').map((button) => button.text())).toEqual(['停止'])
  await wrapper.get('button').trigger('click')
  expect(db.stopRedisKeyLoad).toHaveBeenCalledWith('redis-test::redis:db2::keys::scan-more')
})

it('自动加载仅在可见且启用时请求；隐藏或卸载后迟到观察回调不再请求', async () => {
  FakeIntersectionObserver.instances = []
  vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver)
  const { db } = createDb()
  db.redisAutoLoadEnabled.value = true
  const wrapper = mount(RedisKeyListFooter, {
    props: { db, nodeId: 'redis-test::redis:db2::keys::scan-more', active: true },
  })
  await nextTick()
  const observer = FakeIntersectionObserver.instances.at(-1)!
  expect(observer.rootMargin).toBe('100px 0px')
  observer.trigger(true)
  await nextTick()
  expect(db.autoLoadMoreRedisKeys).toHaveBeenCalledTimes(1)

  await wrapper.setProps({ active: false })
  await nextTick()
  expect(observer.disconnected).toBe(true)
  observer.trigger(true)
  expect(db.autoLoadMoreRedisKeys).toHaveBeenCalledTimes(1)

  await wrapper.setProps({ active: true })
  await nextTick()
  const activeObserver = FakeIntersectionObserver.instances.at(-1)!
  wrapper.unmount()
  expect(activeObserver.disconnected).toBe(true)
  activeObserver.trigger(true)
  expect(db.autoLoadMoreRedisKeys).toHaveBeenCalledTimes(1)
})

it('用户停止后暂停当前库的滚动自动加载，状态解除后恢复观察', async () => {
  FakeIntersectionObserver.instances = []
  vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver)
  const { db, current } = createDb(state({ automaticPaused: true }))
  db.redisAutoLoadEnabled.value = true
  const wrapper = mount(RedisKeyListFooter, {
    props: { db, nodeId: 'redis-test::redis:db2::keys::scan-more', active: true },
  })
  await nextTick()
  expect(FakeIntersectionObserver.instances).toHaveLength(0)
  expect(wrapper.findAll('button').map((button) => button.text())).toEqual(['更多', '全部'])

  current.value = state({ automaticPaused: false })
  await nextTick()
  const observer = FakeIntersectionObserver.instances.at(-1)!
  observer.trigger(true)
  await nextTick()
  expect(db.autoLoadMoreRedisKeys).toHaveBeenCalledTimes(1)
})
