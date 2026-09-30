import { mount, type VueWrapper } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { KeepAlive, defineComponent, h, nextTick, ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import QueryTab from './QueryTab.vue'
import type { useDatabase } from './useDatabase'

class ResizeObserverMock {
  static instances: ResizeObserverMock[] = []
  readonly observe = vi.fn()
  readonly disconnect = vi.fn()

  constructor(private readonly callback: ResizeObserverCallback) {
    ResizeObserverMock.instances.push(this)
  }

  notify() {
    this.callback([], this as unknown as ResizeObserver)
  }
}

const frames = new Map<number, FrameRequestCallback>()
const mounted: Array<{ wrapper: VueWrapper; container: HTMLElement }> = []
let nextFrameId = 0
const childStubs = {
  FixtureSql: true,
  QueryResultPane: true,
  SqlEditor: true,
  UiButton: true,
  UiContextMenu: true,
  UiIcon: true,
  UiIconButton: true,
  UiInput: true,
  UiModal: true,
  UiSelect: true,
  UiToolbar: true,
}

beforeEach(() => {
  setActivePinia(createPinia())
  ResizeObserverMock.instances = []
  frames.clear()
  nextFrameId = 0
  vi.stubGlobal('ResizeObserver', ResizeObserverMock)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = ++nextFrameId
    frames.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    frames.delete(id)
  })
})

afterEach(() => {
  for (const { wrapper, container } of mounted.splice(0)) {
    wrapper.unmount()
    container.remove()
  }
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

function makeDatabase(): ReturnType<typeof useDatabase> {
  const state = ref({
    status: 'idle' as const,
    sql: '',
    rows: [] as string[][],
    values: [] as never[][],
    columns: [] as string[],
    statements: [],
    activeStatement: 0,
    resultTab: 'data',
    selection: null,
    total: 0,
    durationMs: 0,
    truncated: false,
    hasMore: false,
    loadingMore: false,
    paginationMode: undefined,
    gridPage: 1,
    gridPageSize: 100,
    gridEdits: undefined,
    gridSaving: false,
    transactionActive: false,
    savedId: undefined,
    dirty: false,
    filePath: undefined,
    filter: '',
    page: 1,
    cursorBufferStart: 0,
    cancelRequested: false,
    loadMoreError: '',
    loadLimit: '',
    error: '',
  })

  return {
    queryState: state,
    patchQueryState: vi.fn(),
    activeTabContext: ref({ connectionId: '', database: '', schema: '' }),
    activeTabConnection: ref(undefined),
    activeTabId: ref('query-1'),
    activeTabKind: ref('query'),
    activeTab: ref({ label: 'SQL 编辑器' }),
    tabs: ref([{ id: 'query-1', kind: 'query' }]),
    connections: ref([]),
    completionTables: ref([]),
    connectionOptions: ref([]),
    databaseOptions: ref([]),
    schemaOptions: ref([]),
    pageRows: ref([]),
    resultTabs: ref([
      { value: 'data', label: '数据' },
      { value: 'message', label: '消息' },
    ]),
    filteredRows: {
      value: [],
      busy: ref(false),
      error: ref(''),
      ready: vi.fn(async () => []),
    },
    showError: vi.fn(),
    runQuery: vi.fn(),
    cancelQuery: vi.fn(),
    onFormatSql: vi.fn(),
    saveQueryToDisk: vi.fn(async () => undefined),
    closeQueryResults: vi.fn(async () => undefined),
    renameActiveTab: vi.fn(),
    openSqlEditorWithSql: vi.fn(),
    openStructureForTable: vi.fn(),
    resolveEditorColumns: vi.fn(async () => []),
  } as unknown as ReturnType<typeof useDatabase>
}

function mountQueryTab(initialHeight: number, keepAlive = false) {
  const hostHeight = ref(initialHeight)
  const visible = ref(true)
  const db = makeDatabase()
  const container = document.createElement('div')
  document.body.appendChild(container)

  const Host = defineComponent({
    setup() {
      return () =>
        h(
          'main',
          {
            onVnodeBeforeMount: (vnode) => {
              Object.defineProperty(vnode.el, 'clientHeight', {
                configurable: true,
                get: () => hostHeight.value,
              })
            },
          },
          keepAlive
            ? [
                h(KeepAlive, null, {
                  default: () => (visible.value ? h(QueryTab, { db }) : null),
                }),
              ]
            : [h(QueryTab, { db })]
        )
    },
  })
  const wrapper = mount(Host, { attachTo: container, global: { stubs: childStubs } })
  mounted.push({ wrapper, container })
  const editorPane = () => wrapper.get('div.flex.min-h-0.flex-col')
  const editorBasis = () =>
    Number.parseFloat(
      editorPane()
        .attributes('style')
        ?.match(/flex-basis:\s*([0-9.]+)px/)?.[1] ?? 'NaN'
    )

  return {
    wrapper,
    container,
    host: () => wrapper.get('main').element,
    editorPane,
    editorBasis,
    setHeight(value: number) {
      hostHeight.value = value
    },
    setVisible(value: boolean) {
      visible.value = value
    },
    unmount() {
      wrapper.unmount()
      container.remove()
      const index = mounted.findIndex((entry) => entry.wrapper === wrapper)
      if (index >= 0) mounted.splice(index, 1)
    },
  }
}

async function flushFrames() {
  const pending = [...frames.values()]
  frames.clear()
  for (const callback of pending) callback(0)
  await nextTick()
}

function hostObserver(view: ReturnType<typeof mountQueryTab>) {
  const observer = ResizeObserverMock.instances.find(
    (instance) =>
      !instance.disconnect.mock.calls.length &&
      instance.observe.mock.calls.some(([target]) => target === view.host())
  )
  expect(observer).toBeDefined()
  return observer!
}

describe('SQL 编辑器与结果区的高度边界', () => {
  it('首次宿主为零高度时保留安全的编辑器高度', async () => {
    const view = mountQueryTab(0)
    await nextTick()

    expect(view.editorBasis()).toBe(320)
    await flushFrames()
    expect(view.editorBasis()).toBe(320)

    view.setHeight(600)
    hostObserver(view).notify()
    await flushFrames()
    expect(view.editorBasis()).toBe(228)
  })

  it('宿主过矮时初始高度与拖动上限都保留至少 120px', async () => {
    const view = mountQueryTab(200)
    await nextTick()
    await flushFrames()
    expect(view.editorBasis()).toBe(120)

    await view.wrapper.get('.cursor-row-resize').trigger('mousedown', { clientY: 100 })
    window.dispatchEvent(new MouseEvent('mousemove', { clientY: 1000 }))
    await nextTick()
    expect(view.editorBasis()).toBe(120)
    window.dispatchEvent(new MouseEvent('mouseup'))
  })

  it('首次测量前发生手动拖动时不重置，后续缩放只按当前边界钳制', async () => {
    const view = mountQueryTab(500)
    await nextTick()

    await view.wrapper.get('.cursor-row-resize').trigger('mousedown', { clientY: 100 })
    window.dispatchEvent(new MouseEvent('mousemove', { clientY: 30 }))
    await nextTick()
    expect(view.editorBasis()).toBe(250)
    window.dispatchEvent(new MouseEvent('mouseup'))

    await flushFrames()
    expect(view.editorBasis()).toBe(250)

    view.setHeight(0)
    hostObserver(view).notify()
    await flushFrames()
    expect(view.editorBasis()).toBe(250)

    view.setHeight(300)
    hostObserver(view).notify()
    await flushFrames()
    expect(view.editorBasis()).toBe(160)

    view.setHeight(600)
    hostObserver(view).notify()
    await flushFrames()
    expect(view.editorBasis()).toBe(160)
  })

  it('KeepAlive 停用时释放观察器，恢复后保留手动高度并重建观察', async () => {
    const view = mountQueryTab(600, true)
    await nextTick()
    await flushFrames()
    expect(view.editorBasis()).toBe(228)
    const firstObserver = hostObserver(view)

    await view.wrapper.get('.cursor-row-resize').trigger('mousedown', { clientY: 100 })
    window.dispatchEvent(new MouseEvent('mousemove', { clientY: 150 }))
    await nextTick()
    window.dispatchEvent(new MouseEvent('mouseup'))
    expect(view.editorBasis()).toBe(278)

    firstObserver.notify()
    expect(frames.size).toBe(1)
    view.setVisible(false)
    await nextTick()
    expect(firstObserver.disconnect).toHaveBeenCalledOnce()
    expect(frames.size).toBe(0)
    firstObserver.notify()
    expect(frames.size).toBe(0)

    view.setHeight(0)
    view.setVisible(true)
    await nextTick()
    const resumedObserver = hostObserver(view)
    expect(resumedObserver).not.toBe(firstObserver)
    await flushFrames()
    expect(view.editorBasis()).toBe(278)

    view.setHeight(700)
    resumedObserver.notify()
    await flushFrames()
    expect(view.editorBasis()).toBe(278)

    resumedObserver.notify()
    expect(frames.size).toBe(1)
    view.unmount()
    expect(resumedObserver.disconnect).toHaveBeenCalledOnce()
    expect(frames.size).toBe(0)
  })
})
