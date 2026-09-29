import { mount } from '@vue/test-utils'
import { reactive, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import QueryResultPane from './QueryResultPane.vue'
import UiSelect from '@/core/ui/UiSelect.vue'
import type { QueryState, useDatabase } from './useDatabase'

const wrappers: Array<ReturnType<typeof mount>> = []

afterEach(() => {
  wrappers.forEach((wrapper) => wrapper.unmount())
  wrappers.length = 0
})

function makeState(overrides: Partial<QueryState> = {}): QueryState {
  return reactive({
    status: 'success',
    resultTab: 'data',
    statements: [],
    activeStatement: 0,
    columns: ['id'],
    rows: [],
    values: [],
    filter: '',
    gridPage: 1,
    gridPageSize: 100,
    gridEdits: {},
    total: 200,
    durationMs: 1,
    hasMore: true,
    cursorId: 'cursor',
    loadLimit: '达到测试上限',
    loadingMore: false,
    gridSaving: false,
    loadMoreError: '',
    truncated: false,
    error: '',
    cancelRequested: false,
    ...overrides,
  }) as unknown as QueryState
}

it('结果页签可翻阅缓存，选中页大小后保留服务端当前页并受草稿保护', async () => {
  const first = makeState()
  const filtered = Array.from({ length: 200 }, (_, index) => [`row-${index}`])
  const queryStates = ref({ query: first })
  const db = {
    resultTabs: ref([{ value: 'data', label: '数据' }]),
    filteredRows: {
      value: filtered,
      busy: ref(false),
      error: ref(''),
      ready: vi.fn(async () => filtered),
    },
    queryStates,
    activeTabId: ref('query'),
    goToPage: vi.fn(),
    loadMore: vi.fn(),
    showError: vi.fn(),
  } as unknown as ReturnType<typeof useDatabase>
  const wrapper = mount(QueryResultPane, {
    props: {
      db,
      queryState: first,
      rows: [],
      statusText: '查询完成',
    },
    global: { stubs: { EditableResultGrid: true, UiTabs: true } },
  })
  wrappers.push(wrapper)

  const next = () => wrapper.findAll('button').find((button) => button.text() === '下一页')!
  expect(next().attributes('disabled')).toBeUndefined()
  await next().trigger('click')
  expect(wrapper.emitted('patch')?.at(-1)).toEqual([{ gridPage: 2 }])
  expect(db.loadMore).not.toHaveBeenCalled()

  const second = makeState({ gridPage: 2, loadLimit: '', paginationMode: 'server' })
  queryStates.value = { query: second }
  await wrapper.setProps({ queryState: second, rows: [{ __row: '100', c0: 'row-100' }] })
  expect(wrapper.text()).toContain('第 2 页')

  const pageSize = wrapper.findComponent(UiSelect)
  expect(wrapper.find('button[aria-label="每页条数"]').exists()).toBe(true)
  expect(pageSize.props('options').map(({ value }) => value)).toEqual(['100', '200', '500', '1000'])
  await pageSize.vm.$emit('update:modelValue', '500')
  expect(db.goToPage).toHaveBeenCalledWith('query', 2, 500)
  expect(wrapper.text()).not.toContain('应用')
  expect(wrapper.text()).not.toContain('读取当前页剩余数据')

  await wrapper.setProps({
    queryState: makeState({ gridEdits: { 0: { c0: { kind: 'text', value: 'draft' } } } }),
  })
  expect(pageSize.props('disabled')).toBe(true)
  expect(next().attributes('disabled')).toBeDefined()
})

it('页码 Enter 跳转，非法页码不请求，首页和尾页调用目标接口', async () => {
  const state = makeState({ gridPage: 2, paginationMode: 'server', loadLimit: '' })
  const db = {
    resultTabs: ref([{ value: 'data', label: '数据' }]),
    filteredRows: {
      value: Array.from({ length: 200 }, (_, index) => [`row-${index}`]),
      busy: ref(false),
      error: ref(''),
      ready: vi.fn(async () => []),
    },
    queryStates: ref({ query: state }),
    activeTabId: ref('query'),
    goToPage: vi.fn(async () => true),
    jumpToPage: vi.fn(async () => true),
    goToLastPage: vi.fn(async () => true),
    loadMore: vi.fn(),
    showError: vi.fn(),
  } as unknown as ReturnType<typeof useDatabase>
  const wrapper = mount(QueryResultPane, {
    props: {
      db,
      queryState: state,
      rows: [],
      statusText: '查询完成',
    },
    global: { stubs: { EditableResultGrid: true, UiTabs: true } },
  })
  wrappers.push(wrapper)

  const input = wrapper.get<HTMLInputElement>('input[aria-label="跳转页码"]')
  await input.setValue('4')
  await input.trigger('keydown.enter')
  expect(db.jumpToPage).toHaveBeenCalledWith('query', 4, 100)

  await input.setValue('0')
  await input.trigger('keydown.enter')
  expect(db.jumpToPage).toHaveBeenCalledTimes(1)
  expect(wrapper.get('[role="alert"]').text()).toContain('页码需在 1 至 4294967295 之间')

  await wrapper.get('button[aria-label="首页"]').trigger('click')
  expect(db.goToPage).toHaveBeenCalledWith('query', 1)
  await wrapper.get('button[aria-label="尾页"]').trigger('click')
  expect(db.goToLastPage).toHaveBeenCalledWith('query', 100)
})
