import { mount, flushPromises } from '@vue/test-utils'
import { computed, markRaw, nextTick, reactive, ref, toRaw } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import ResultCanvasGrid from './ResultCanvasGrid.vue'
import EditableResultGrid from './EditableResultGrid.vue'
import { indexResultRows, resultGridRows } from './resultRows'
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
    activeTabConnection: ref({ dbType: 'postgresql' }),
    tabs: ref([{ id: 'q', kind: 'query' }]),
    tableColumns: computed(() =>
      state.columns.map((label, index) => ({ key: `c${index}`, label }))
    ),
    patchTabQueryState: (_id: string, patch: Partial<QueryState>) => Object.assign(state, patch),
    loadTableData: vi.fn(),
  } as unknown as ReturnType<typeof useDatabase>
  const wrapper = mount(EditableResultGrid, {
    global: {
      stubs: {
        UiModal: {
          props: ['open'],
          template: '<div v-if="open"><slot /><slot name="footer" /></div>',
        },
      },
    },
    props: { db, state, rows: [{ __row: '0', c0: '9007199254740993', c1: '原始值' }] },
  })
  wrappers.push(wrapper)
  return { wrapper, state, db }
}
async function triggerCell(wrapper: ReturnType<typeof mount>, event: 'cell' | 'context') {
  const grid = wrapper.getComponent(ResultCanvasGrid)
  const payload = { row: grid.props('rows')[0], column: grid.props('columns')[1] }
  if (event === 'context')
    grid.vm.$emit('context', payload, new MouseEvent('contextmenu', { clientX: 100, clientY: 100 }))
  else grid.vm.$emit('cell', payload)
  await nextTick()
}
async function edit(wrapper: ReturnType<typeof mount>, text: string) {
  await flushPromises()
  await triggerCell(wrapper, 'cell')
  await nextTick()
  await wrapper.get('input[aria-label="编辑 name"]').setValue(text)
}
function button(wrapper: ReturnType<typeof mount>, name: string) {
  return wrapper.findAll('button').find((button) => button.text() === name)!
}
async function rowMenu(wrapper: ReturnType<typeof mount>, label: string) {
  await triggerCell(wrapper, 'context')
  await flushPromises()
  const action = Array.from(document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')).find(
    (item) => item.textContent?.trim() === label
  )!
  action.click()
  await nextTick()
}
describe('结果单元格编辑事务', () => {
  it('长字段网格仅显示短预览，完整值与复制保留原文', async () => {
    const { wrapper, state } = setup()
    const text = '长'.repeat(20_000)
    state.rows[0][1] = text
    state.values[0][1] = { kind: 'text', value: text }
    await flushPromises()
    expect(wrapper.getComponent(ResultCanvasGrid).props('text')(0, 1)).toBe('长'.repeat(500) + '…')
    expect(state.values[0][1].value).toBe(text)
    await rowMenu(wrapper, '复制单元格')
    expect(mocks.copy).toHaveBeenLastCalledWith(text)
  })
  it('查询快照转为原始数组后，可见代理行仍显示原值并编辑、复制正确行', async () => {
    const { wrapper, state } = setup()
    const visible = state.rows.slice()
    state.rows = markRaw(toRaw(state.rows))
    await wrapper.setProps({
      rows: resultGridRows(state.columns, visible, state.values, indexResultRows(state.rows)),
    })
    await flushPromises()
    expect(wrapper.getComponent(ResultCanvasGrid).props('text')(0, 0)).toBe('9007199254740993')
    expect(wrapper.getComponent(ResultCanvasGrid).props('text')(0, 1)).toBe('原始值')
    expect(wrapper.text()).not.toContain('(空字符串)')
    await edit(wrapper, '修改后')
    await button(wrapper, '完成').trigger('click')
    expect(state.gridEdits?.[0].name.value).toBe('修改后')
    await rowMenu(wrapper, '复制整行')
    expect(mocks.copy).toHaveBeenLastCalledWith('9007199254740993\t修改后', '已复制整行')
  })
  it('无明确来源的查询结果仍可复制 SQL，并明确提示占位表名', async () => {
    const { wrapper, state } = setup()
    state.editTarget = null
    await flushPromises()
    await rowMenu(wrapper, '复制为 SQL')
    expect(mocks.copy).toHaveBeenLastCalledWith(
      `INSERT INTO "result" ("id", "name") VALUES (9007199254740993, E'原始值');`,
      '已复制 INSERT SQL，请替换占位表名 result'
    )
    expect(mocks.apply).not.toHaveBeenCalled()
  })
  it('从右键菜单复制整行与 INSERT，使用当前草稿且不触发数据库写入', async () => {
    const { wrapper } = setup()
    await edit(wrapper, '修改值')
    await button(wrapper, '完成').trigger('click')
    await rowMenu(wrapper, '复制整行')
    expect(mocks.copy).toHaveBeenLastCalledWith('9007199254740993\t修改值', '已复制整行')
    await rowMenu(wrapper, '复制为 SQL')
    expect(mocks.copy).toHaveBeenLastCalledWith(
      `INSERT INTO "public"."users" ("id", "name") VALUES (9007199254740993, E'修改值');`,
      '已复制 INSERT SQL'
    )
    expect(mocks.apply).not.toHaveBeenCalled()
    expect(button(wrapper, '复制整行')).toBeUndefined()
  })
  it('双击弹窗编辑，完成不写库，手动保存保留精确主键和原值', async () => {
    const { wrapper, state } = setup()
    expect(button(wrapper, '保存并提交')).toBeUndefined()
    await edit(wrapper, '修改值')
    expect(mocks.apply).not.toHaveBeenCalled()
    await button(wrapper, '完成').trigger('click')
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
    expect(button(wrapper, '保存并提交')).toBeUndefined()
    expect(wrapper.text()).toContain('已提交 1 行')
    expect(wrapper.getComponent(ResultCanvasGrid).props('text')(0, 1)).toBe('修改值')
  })
  it('空串不等于 NULL，通过单元格右键菜单设置空值', async () => {
    const { wrapper, state } = setup()
    await edit(wrapper, '')
    expect(state.gridEdits?.[0].name).toEqual({ kind: 'text', value: '' })
    await button(wrapper, '完成').trigger('click')
    await triggerCell(wrapper, 'context')
    await flushPromises()
    const nullAction = Array.from(
      document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')
    ).find((item) => item.textContent?.trim() === '设为 NULL')!
    nullAction.click()
    await nextTick()
    expect(state.gridEdits?.[0].name).toEqual({ kind: 'null', value: null })
  })
  it('保存冲突保留草稿，结果未知禁止直接重试', async () => {
    const { wrapper, state } = setup()
    mocks.apply.mockRejectedValueOnce(new Error('原值冲突，已回滚'))
    await edit(wrapper, '重试内容')
    await button(wrapper, '完成').trigger('click')
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
    await triggerCell(wrapper, 'cell')
    expect(wrapper.find('input').exists()).toBe(false)
    expect(button(wrapper, '编辑')).toBeUndefined()
    expect(wrapper.text()).not.toContain('只读结果')
  })
})
