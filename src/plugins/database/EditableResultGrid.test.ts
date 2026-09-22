import { mount, flushPromises } from '@vue/test-utils'
import { computed, nextTick, reactive, ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import EditableResultGrid from './EditableResultGrid.vue'
import type { QueryState, useDatabase } from './useDatabase'

const mocks = vi.hoisted(() => ({ columns: vi.fn(), apply: vi.fn(), copy: vi.fn() }))
vi.mock('./ipc', () => ({ queryIpc: { columns: mocks.columns }, tableIpc: { apply: mocks.apply } }))
vi.mock('@/core/feedback/useCopy', () => ({ useCopy: () => ({ copyText: mocks.copy }) }))
const wrappers: Array<ReturnType<typeof mount>> = []
beforeEach(() => {
  vi.clearAllMocks()
  mocks.columns.mockResolvedValue([
    { name: 'id', dataType: 'bigint', key: 'PK' },
    { name: 'name', dataType: 'text', key: '' },
  ])
  mocks.apply.mockResolvedValue(1)
})
afterEach(() => {
  wrappers.forEach((wrapper) => wrapper.unmount())
  wrappers.length = 0
})
function setup() {
  const state = reactive({
    editTarget: { connId: 'c', database: 'app', schema: 'public', table: 'users' },
    rows: [['9007199254740993', '原始值']],
    columns: ['id', 'name'],
    values: [
      [
        { kind: 'integer', value: '9007199254740993' },
        { kind: 'text', value: '原始值' },
      ],
    ],
    status: 'success',
    selectedRow: '',
  }) as QueryState
  const db = {
    connections: ref([
      { id: 'c', label: '测试库', dbType: 'postgresql', status: 'online', readonly: false },
    ]),
    queryStates: ref({ q: state }),
    activeTabId: ref('q'),
    tabs: ref([{ id: 'q', kind: 'query' }]),
    tableColumns: computed(() =>
      state.columns.map((label, index) => ({ key: `c${index}`, label }))
    ),
    patchTabQueryState: (_id: string, patch: Partial<QueryState>) => Object.assign(state, patch),
    loadTableData: vi.fn(),
  } as unknown as ReturnType<typeof useDatabase>
  const wrapper = mount(EditableResultGrid, {
    props: { db, state, rows: [{ __row: '0', c0: '9007199254740993', c1: '原始值' }] },
  })
  wrappers.push(wrapper)
  return { wrapper, state, db }
}
async function edit(wrapper: ReturnType<typeof mount>, text: string) {
  await flushPromises()
  await wrapper.findAll('td')[2].trigger('dblclick')
  await nextTick()
  await wrapper.get('input[aria-label="编辑 name"]').setValue(text)
}
function button(wrapper: ReturnType<typeof mount>, name: string) {
  return wrapper.findAll('button').find((button) => button.text() === name)!
}
describe('结果单元格编辑事务', () => {
  it('双击就地编辑，失焦不写库，手动保存保留精确主键和原值', async () => {
    const { wrapper, state } = setup()
    await edit(wrapper, '修改值')
    expect(mocks.apply).not.toHaveBeenCalled()
    await wrapper.get('input').trigger('blur')
    expect(wrapper.find('input').exists()).toBe(false)
    expect(state.gridEdits?.[0].name.value).toBe('修改值')
    await button(wrapper, '保存并提交').trigger('click')
    await flushPromises()
    expect(mocks.apply).toHaveBeenCalledWith(
      expect.objectContaining({ table: 'users' }),
      [
        {
          action: 'update',
          values: { name: { kind: 'text', value: '修改值' } },
          original: {
            id: { kind: 'integer', value: '9007199254740993' },
            name: { kind: 'text', value: '原始值' },
          },
        },
      ],
      expect.any(String)
    )
    expect(state.gridEdits).toEqual({})
    expect(wrapper.text()).toContain('已提交 1 行')
    expect(wrapper.text()).toContain('修改值')
  })
  it('空串不等于 NULL，通过按钮设置空值', async () => {
    const { wrapper, state } = setup()
    await edit(wrapper, '')
    expect(state.gridEdits?.[0].name).toEqual({ kind: 'text', value: '' })
    await wrapper.get('input').trigger('blur')
    await button(wrapper, '设为 NULL').trigger('click')
    expect(state.gridEdits?.[0].name).toEqual({ kind: 'null', value: null })
  })
  it('保存冲突保留草稿，结果未知禁止直接重试', async () => {
    const { wrapper, state } = setup()
    mocks.apply.mockRejectedValueOnce(new Error('原值冲突，已回滚'))
    await edit(wrapper, '重试内容')
    await wrapper.get('input').trigger('blur')
    await button(wrapper, '保存并提交').trigger('click')
    await flushPromises()
    expect(state.gridEdits?.[0].name.value).toBe('重试内容')
    expect(wrapper.text()).toContain('原值冲突')
    mocks.apply.mockRejectedValueOnce(new Error('DB_OUTCOME_UNKNOWN: 提交超时'))
    await button(wrapper, '保存并提交').trigger('click')
    await flushPromises()
    expect(button(wrapper, '保存并提交').attributes('disabled')).toBeDefined()
    expect(state.gridEdits?.[0].name.value).toBe('重试内容')
  })
  it('缺主键或只读连接不开放双击编辑', async () => {
    mocks.columns.mockResolvedValue([
      { name: 'id', dataType: 'bigint', key: '' },
      { name: 'name', dataType: 'text', key: '' },
    ])
    const { wrapper } = setup()
    await flushPromises()
    await wrapper.findAll('td')[2].trigger('dblclick')
    expect(wrapper.find('input').exists()).toBe(false)
    expect(wrapper.text()).toContain('没有主键')
  })
})
