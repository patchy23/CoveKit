import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { defineComponent, h, nextTick, ref } from 'vue'
import { publishToolVisibility, resetToolVisibilityForTest } from '@/core/lifecycle'
import type { UiTreeItem } from '@/core/ui'
import type { DbConnectionInfo } from './contracts'
import type { useDatabase } from './useDatabase'
import ConnectionsSidebar from './ConnectionsSidebar.vue'

vi.mock('@/core/feedback/useCopy', () => ({ useCopy: () => ({ copyText: vi.fn() }) }))
enableAutoUnmount(afterEach)

class FakeIntersectionObserver {
  static instances: FakeIntersectionObserver[] = []
  readonly callback: IntersectionObserverCallback
  observed: Element | null = null
  disconnected = false

  constructor(callback: IntersectionObserverCallback) {
    this.callback = callback
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

  trigger() {
    if (!this.observed) return
    this.callback(
      [{ target: this.observed, isIntersecting: true } as IntersectionObserverEntry],
      this as unknown as IntersectionObserver
    )
  }
}

const SearchStub = defineComponent({
  setup(_, { slots }) {
    return () => h('div', [slots.default?.(), slots.actions?.()])
  },
})

function redisState(overrides: Record<string, unknown> = {}) {
  return {
    connId: 'redis-test',
    database: 'db1',
    loaded: 2,
    total: 10,
    hasMore: true,
    loading: false,
    fetchingAll: false,
    error: '',
    limitReached: false,
    limitReason: null,
    autoLoadBudgetReached: false,
    ...overrides,
  }
}

function setup() {
  const connection: DbConnectionInfo = {
    id: 'redis-test',
    label: '测试 Redis',
    dbType: 'redis',
    env: 'test',
    status: 'online',
    version: '',
    latencyMs: 1,
    readonly: false,
    host: '127.0.0.1',
    database: 'db0',
    connectedAt: 0,
    port: 6379,
    username: '',
    ssl: false,
    connectTimeoutMs: 8000,
  }
  const footer: UiTreeItem = {
    id: 'redis-test::redis:db1::keys::scan-more',
    label: '键列表',
    depth: 3,
    kind: 'redis-key-footer',
  }
  const selectedResource = ref('some-other-selection')
  const redisAutoLoadEnabled = ref(true)
  const db = {
    connections: ref([connection]),
    treeItems: ref([footer]),
    visibleTreeItems: ref([footer]),
    selectedResource,
    keyword: ref(''),
    activeConnectionId: ref(connection.id),
    connecting: ref({}),
    connectError: ref({}),
    redisAutoLoadEnabled,
    redisKeyLoadState: vi.fn(() => redisState()),
    loadMoreRedisKeys: vi.fn(async () => {}),
    fetchAllRedisKeys: vi.fn(async () => {}),
    stopRedisKeyLoad: vi.fn(),
    autoLoadMoreRedisKeys: vi.fn(async () => {}),
    pauseRedisKeyLoads: vi.fn(),
    cancelConnect: vi.fn(),
    connect: vi.fn(async () => {}),
    disconnect: vi.fn(async () => {}),
    saveConnection: vi.fn(),
    removeConnection: vi.fn(),
    refreshConnections: vi.fn(),
    parseLeafId: vi.fn(() => null),
    scopeContext: vi.fn(() => ({ database: 'db1', schema: '' })),
    openSqlEditorWithSql: vi.fn(),
    openSqlEditor: vi.fn(),
    openCreateTableTab: vi.fn(),
    openCreateTableEditor: vi.fn(),
    dropDatabase: vi.fn(async () => true),
    selectResource: vi.fn(),
    openStructureTab: vi.fn(),
    tableAdminAction: vi.fn(async () => true),
    showSystemSchemas: ref({}),
    toggleSystemSchemas: vi.fn(),
  } as unknown as ReturnType<typeof useDatabase>
  const wrapper = mount(ConnectionsSidebar, {
    props: { db },
    global: {
      stubs: {
        UiSearchInput: SearchStub,
        UiConfirmDialog: true,
        UiModal: true,
        UiInput: true,
        UiContextMenu: true,
        CreateDatabaseDialog: true,
      },
    },
  })
  return { wrapper, db, selectedResource }
}

beforeEach(() => {
  FakeIntersectionObserver.instances = []
  vi.stubGlobal('IntersectionObserver', FakeIntersectionObserver)
  resetToolVisibilityForTest()
  publishToolVisibility('database', { active: true, covered: false, hidden: false })
})
afterEach(() => {
  vi.unstubAllGlobals()
  resetToolVisibilityForTest()
})

it('滚动自动加载服从工具可见性，按钮不改变当前选择', async () => {
  const { wrapper, db, selectedResource } = setup()
  await nextTick()
  const observer = FakeIntersectionObserver.instances.at(-1)!
  expect(observer).toBeDefined()
  const fetchAllButton = wrapper.findAll('button').find((button) => button.text() === '全部')
  expect(fetchAllButton).toBeDefined()
  await fetchAllButton!.trigger('click')
  expect(db.fetchAllRedisKeys).toHaveBeenCalledWith('redis-test::redis:db1::keys::scan-more')
  expect(selectedResource.value).toBe('some-other-selection')

  observer.trigger()
  await flushPromises()
  expect(db.autoLoadMoreRedisKeys).toHaveBeenCalledTimes(1)

  publishToolVisibility('database', { hidden: true })
  await nextTick()
  expect(db.pauseRedisKeyLoads).toHaveBeenCalledTimes(1)
  expect(observer.disconnected).toBe(true)
  observer.trigger()
  expect(db.autoLoadMoreRedisKeys).toHaveBeenCalledTimes(1)
})
