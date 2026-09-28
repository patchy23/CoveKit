import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import UiDataGrid from './UiDataGrid.vue'
import UiTooltip from './UiTooltip.vue'
import { nextTick } from 'vue'

enableAutoUnmount(afterEach)
afterEach(() => vi.restoreAllMocks())

const columns = [
  { key: 'a', label: 'A', width: 100 },
  { key: 'b', label: 'B', width: 120 },
]
const move = () => window.dispatchEvent(new PointerEvent('pointermove', { clientX: 80 }))

it('保留仍展示列的用户宽度，移除列后不再持有其历史宽度', async () => {
  const grid = mount(UiDataGrid, { props: { columns, rows: [], rowNumbers: false } })
  await grid.get('th span.cursor-col-resize').trigger('pointerdown', { clientX: 20 })
  move()
  window.dispatchEvent(new PointerEvent('pointerup'))
  await grid.vm.$nextTick()
  expect((grid.get('th').element as HTMLElement).style.width).toBe('160px')
  await grid.setProps({ columns: [{ key: 'a', label: '新标题', width: 200 }] })
  expect((grid.get('th').element as HTMLElement).style.width).toBe('160px')
  await grid.setProps({ columns: [{ key: 'b', label: 'B', width: 180 }] })
  expect((grid.get('th').element as HTMLElement).style.width).toBe('180px')
  await grid.setProps({ columns })
  expect((grid.get('th').element as HTMLElement).style.width).toBe('100px')
})

for (const reason of ['pointercancel', 'remove-column', 'unmount']) {
  it(`拖动在 ${reason} 时释放监听器且后续移动不再修改宽度`, async () => {
    const grid = mount(UiDataGrid, { props: { columns, rows: [], rowNumbers: false } })
    const remove = vi.spyOn(window, 'removeEventListener')
    await grid.get('th span.cursor-col-resize').trigger('pointerdown', { clientX: 20 })
    if (reason === 'unmount') grid.unmount()
    else if (reason === 'remove-column') await grid.setProps({ columns: [columns[1]] })
    else window.dispatchEvent(new PointerEvent('pointercancel'))
    for (const name of ['pointermove', 'pointerup', 'pointercancel', 'blur']) {
      expect(remove).toHaveBeenCalledWith(name, expect.any(Function))
    }
    move()
    if (reason !== 'unmount') {
      await grid.vm.$nextTick()
      expect((grid.get('th').element as HTMLElement).style.width).toBe(
        reason === 'remove-column' ? '120px' : '100px'
      )
    }
  })
}

const makeRows = (count: number) =>
  Array.from({ length: count }, (_, index) => ({
    id: String(index),
    a: `值 ${index}`,
    b: index,
  }))

it('大结果只挂载窗口内单元格，长字段预览有界但操作保留原值', async () => {
  const text = '长字段'.repeat(10000)
  const grid = mount(UiDataGrid, {
    props: {
      columns,
      rows: [
        { id: '0', a: text, b: 0 },
        ...makeRows(50000).map((row, i) => ({ ...row, id: String(i + 1) })),
      ],
      virtual: true,
    },
  })
  await nextTick()
  expect(grid.findAll('[data-grid-row]').length).toBeLessThan(150)
  const cell = grid.get('[data-grid-row="0"] td:nth-child(2)')
  expect(cell.text()).toBe(text.slice(0, 500) + '…')
  await cell.trigger('dblclick')
  expect(grid.emitted('cell')?.[0]?.[0]).toMatchObject({ row: { a: text } })
})

it('默认渲染完整数据且不主动请求后续数据', async () => {
  const grid = mount(UiDataGrid, { props: { columns, rows: makeRows(160) } })
  await nextTick()
  expect(grid.findAll('[data-grid-row]')).toHaveLength(160)
  expect(grid.emitted('near-end')).toBeUndefined()
})

it('单元格数量变化不会创建 UiTooltip 组件', async () => {
  const grid = mount(UiDataGrid, { props: { columns, rows: makeRows(1), virtual: true } })
  expect(grid.findAllComponents(UiTooltip)).toHaveLength(0)
  await grid.setProps({ rows: makeRows(200) })
  await nextTick()
  expect(grid.findAllComponents(UiTooltip)).toHaveLength(0)
})

it('长值 title 最多500字符且双击和右键事件保留原始行值', async () => {
  const text = '界'.repeat(600)
  const row = { id: 'row-1', value: text }
  const column = { key: 'value', label: '值' }
  const grid = mount(UiDataGrid, {
    props: { columns: [column], rows: [row], rowNumbers: false },
  })
  const cell = grid.get('tbody td')
  expect(cell.attributes('title')).toBe(`${text.slice(0, 499)}…`)
  expect(cell.attributes('title')?.length).toBeLessThanOrEqual(500)
  expect(cell.text()).toBe(text)
  await cell.trigger('dblclick')
  expect(grid.emitted('cell')?.[0]).toEqual([{ row, column }])
  const contextMenu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true })
  cell.element.dispatchEvent(contextMenu)
  await nextTick()
  const contextPayload = grid.emitted('cell-contextmenu')?.[0]
  expect(contextPayload?.[0]).toEqual({ row, column })
  expect(contextPayload?.[1]).toBe(contextMenu)
})

it('虚拟网格保持全局行号和插槽数据，键盘可到达最后一行并保留离屏焦点', async () => {
  const grid = mount(UiDataGrid, {
    props: { columns, rows: makeRows(3000), virtual: true },
    slots: { cell: '<span>{{ row.id }}:{{ column.key }}:{{ value }}</span>' },
    attachTo: document.body,
  })
  await nextTick()
  expect(grid.findAll('[data-grid-row]').length).toBeLessThan(150)
  const first = grid.get('[data-grid-row="0"]')
  ;(first.element as HTMLElement).focus()
  await first.trigger('keydown', { key: 'End' })
  await nextTick()
  const last = grid.get('[data-grid-row="2999"]')
  expect(last.get('td').text()).toBe('3000')
  expect(last.text()).toContain('2999:a:值 2999')
  expect(last.attributes('aria-rowindex')).toBe('3001')
  expect(document.activeElement).toBe(last.element)
  expect(grid.findAll('[data-grid-row]').length).toBeLessThan(150)
  const root = grid.get('div[tabindex="0"]')
  ;(root.element as HTMLElement).scrollTop = 0
  await root.trigger('scroll')
  expect(document.activeElement).toBe(last.element)
  await last.trigger('keydown', { key: 'Enter' })
  expect(grid.emitted('update:modelValue')?.at(-1)).toEqual(['2999'])
  await last.trigger('keydown', { key: 'Home' })
  await nextTick()
  expect(document.activeElement).toBe(grid.get('[data-grid-row="0"]').element)
})

it('程序选中离屏行会进入视口，追加数据不重置滚动或当前选择', async () => {
  const rows = makeRows(1000)
  const grid = mount(UiDataGrid, { props: { columns, rows, virtual: true } })
  await grid.setProps({ modelValue: '800' })
  await nextTick()
  const root = grid.get('div[tabindex="0"]').element as HTMLElement
  const top = root.scrollTop
  expect(top).toBeGreaterThan(0)
  expect(grid.get('[data-grid-row="800"]').attributes('aria-selected')).toBe('true')
  await grid.setProps({ rows: makeRows(1100) })
  expect(root.scrollTop).toBe(top)
  expect(grid.get('[data-grid-row="800"]').attributes('aria-selected')).toBe('true')
})

it('接近底部只通知一次，追加数据可继续触发，关闭通知后不再触发', async () => {
  const grid = mount(UiDataGrid, {
    props: { columns, rows: makeRows(100), virtual: true, nearEnd: true, nearEndThreshold: 100 },
  })
  const root = grid.get('div[tabindex="0"]')
  const element = root.element as HTMLElement
  Object.defineProperty(element, 'clientHeight', { configurable: true, value: 250 })
  Object.defineProperty(element, 'scrollHeight', { configurable: true, value: 1000 })
  element.scrollTop = 649
  await root.trigger('scroll')
  expect(grid.emitted('near-end')).toBeUndefined()
  element.scrollTop = 650
  await root.trigger('scroll')
  await root.trigger('scroll')
  expect(grid.emitted('near-end')).toHaveLength(1)
  await grid.setProps({ rows: makeRows(110) })
  await nextTick()
  expect(grid.emitted('near-end')).toHaveLength(2)
  await grid.setProps({ nearEnd: false })
  await root.trigger('scroll')
  expect(grid.emitted('near-end')).toHaveLength(2)
})

it('缩小结果集后清理旧焦点索引并回到现有行', async () => {
  const grid = mount(UiDataGrid, {
    props: { columns, rows: makeRows(500), virtual: true },
    attachTo: document.body,
  })
  await nextTick()
  const first = grid.get('[data-grid-row="0"]')
  ;(first.element as HTMLElement).focus()
  await first.trigger('keydown', { key: 'End' })
  await nextTick()
  await grid.setProps({ rows: makeRows(3) })
  await nextTick()
  expect(grid.findAll('[data-grid-row]')).toHaveLength(3)
  expect(document.activeElement).toBe(grid.get('[data-grid-row="0"]').element)
})

it('滚动保留正在编辑的输入框，单元格方向键不被行导航接管', async () => {
  const grid = mount(UiDataGrid, {
    props: { columns, rows: makeRows(1000), virtual: true },
    slots: { 'cell-a': '<input :aria-label="`编辑 ${row.id}`" :value="value" />' },
    attachTo: document.body,
  })
  await nextTick()
  const input = grid.get('input[aria-label="编辑 0"]')
  ;(input.element as HTMLElement).focus()
  await input.trigger('keydown', { key: 'ArrowDown' })
  expect(document.activeElement).toBe(input.element)
  expect(grid.emitted('update:modelValue')).toBeUndefined()
  const root = grid.get('div[tabindex="0"]')
  ;(root.element as HTMLElement).scrollTop = 15000
  await root.trigger('scroll')
  expect(document.activeElement).toBe(input.element)
  expect(grid.get('input[aria-label="编辑 0"]').element).toBe(input.element)
  expect(grid.findAll('[data-grid-row]').length).toBeLessThan(150)
})
