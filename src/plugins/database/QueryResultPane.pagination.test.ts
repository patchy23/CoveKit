import { mount } from '@vue/test-utils'
import { reactive, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import QueryResultPane from './QueryResultPane.vue'
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

it('结果页签可翻阅缓存且响应替换的查询状态，调页大小需显式应用并受草稿保护', async () => {
  const first = makeState()
  const filtered = Array.from({ length: 200 }, (_, index) => [`row-${index}`])
  const db = {
    resultTabs: ref([{ value: 'data', label: '数据' }]),
    filteredRows: {
      value: filtered,
      busy: ref(false),
      error: ref(''),
      ready: vi.fn(async () => filtered),
    },
    queryStates: ref({ query: first }),
    activeTabId: ref('query'),
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

  const second = makeState({ gridPage: 2, loadLimit: '' })
  await wrapper.setProps({ queryState: second, rows: [{ __row: '100', c0: 'row-100' }] })
  expect(wrapper.text()).toContain('第 2 页')

  const pageSize = wrapper.get('input[aria-label="每页条数"]')
  await pageSize.setValue('50')
  expect(wrapper.emitted('patch')?.at(-1)).toEqual([{ gridPage: 2 }])
  await wrapper
    .findAll('button')
    .find((button) => button.text() === '应用')!
    .trigger('click')
  expect(wrapper.emitted('patch')?.at(-1)).toEqual([{ gridPage: 1, gridPageSize: 50 }])

  await wrapper.setProps({
    queryState: makeState({ gridEdits: { 0: { c0: { kind: 'text', value: 'draft' } } } }),
  })
  expect(
    wrapper
      .findAll('button')
      .find((button) => button.text() === '应用')!
      .attributes('disabled')
  ).toBeDefined()
  expect(next().attributes('disabled')).toBeDefined()
})
