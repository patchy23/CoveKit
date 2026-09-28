import { mount } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import ResultCanvasGrid from './ResultCanvasGrid.vue'

const mounted: Array<ReturnType<typeof mount>> = []
afterEach(() => {
  for (const wrapper of mounted) wrapper.unmount()
  mounted.length = 0
})
function setup(count = 100, columns = 91) {
  const wrapper = mount(ResultCanvasGrid, {
    props: {
      columns: Array.from({ length: columns }, (_, index) => ({
        key: `c${index}`,
        label: `列${index}`,
        width: 140,
      })),
      rows: Array.from({ length: count }, (_, index) => ({ __row: String(index + 200) })),
      selected: null,
      text: (row, column) => `${row}:${column}`,
      dirty: () => false,
    },
  })
  mounted.push(wrapper)
  return wrapper
}

it('宽表正文只有一个画布，增加结果行数不增加单元格 DOM', async () => {
  const wrapper = setup()
  const nodes = wrapper.findAll('*').length
  expect(wrapper.findAll('canvas')).toHaveLength(1)
  expect(wrapper.findAll('td,input')).toHaveLength(0)
  await wrapper.setProps({
    rows: Array.from({ length: 1000 }, (_, index) => ({ __row: String(index) })),
  })
  expect(wrapper.findAll('*')).toHaveLength(nodes)
  expect(wrapper.attributes('aria-rowcount')).toBe('1000')
})

it('鼠标命中、双击和右键沿用原行身份，表头不触发编辑', async () => {
  const wrapper = setup()
  const canvas = wrapper.get('canvas')
  await canvas.trigger('click', { clientX: 200, clientY: 35 })
  expect(wrapper.emitted('select')?.[0]?.[0]).toMatchObject({
    row: { __row: '200' },
    column: { key: 'c1' },
  })
  await canvas.trigger('dblclick', { clientX: 200, clientY: 35 })
  await canvas.trigger('contextmenu', { clientX: 200, clientY: 35 })
  expect(wrapper.emitted('cell')).toHaveLength(1)
  expect(wrapper.emitted('context')).toHaveLength(1)
  await canvas.trigger('dblclick', { clientX: 200, clientY: 10 })
  expect(wrapper.emitted('cell')).toHaveLength(1)
})

it('键盘选择、编辑和列宽调整无需为单元格挂载输入控件', async () => {
  const wrapper = setup()
  await wrapper.setProps({ selected: { row: 200, column: 1 } })
  await wrapper.trigger('keydown', { key: 'ArrowDown' })
  expect(wrapper.emitted('select')?.[0]?.[0]).toMatchObject({ row: { __row: '201' } })
  await wrapper.trigger('keydown', { key: 'F2' })
  expect(wrapper.emitted('cell')?.[0]?.[0]).toMatchObject({
    row: { __row: '200' },
    column: { key: 'c1' },
  })
  await wrapper.trigger('keydown', { key: 'c', ctrlKey: true })
  expect(wrapper.emitted('copy')).toHaveLength(1)
  const handle = wrapper.findAll('[role="separator"]')[0]
  await handle.trigger('keydown', { key: 'ArrowRight' })
  expect(handle.attributes('aria-valuenow')).toBe('150')
  expect(wrapper.find('input').exists()).toBe(false)
})
