import { mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { h, nextTick } from 'vue'
import ResultCanvasGrid from './ResultCanvasGrid.vue'

const mounted: Array<ReturnType<typeof mount>> = []
afterEach(() => {
  for (const wrapper of mounted) wrapper.unmount()
  mounted.length = 0
  vi.unstubAllGlobals()
})
function setup(
  count = 100,
  columns = 91,
  options: {
    editing?: { row: number; column: number }
    editor?: boolean
    columnTypes?: string[]
  } = {}
) {
  const wrapper = mount(ResultCanvasGrid, {
    props: {
      columns: Array.from({ length: columns }, (_, index) => ({
        key: `c${index}`,
        label: `列${index}`,
        width: 140,
      })),
      columnTypes: options.columnTypes,
      rows: Array.from({ length: count }, (_, index) => ({ __row: String(index + 200) })),
      selected: null,
      editing: options.editing ?? null,
      text: (row, column) => `${row}:${column}`,
      dirty: () => false,
    },
    slots: options.editor
      ? {
          editor: ({ style }: { style: Record<string, string | number> }) =>
            h('input', { 'aria-label': '编辑中', style }),
        }
      : undefined,
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

it('空结果不渲染表头，避免覆盖空状态文案', () => {
  const wrapper = setup(0, 2)
  expect(wrapper.findAll('[role="columnheader"]')).toHaveLength(0)
  expect(wrapper.text()).toContain('暂无数据')
})

it('ResizeObserver 的尺寸测量延迟到下一帧，避免回调中改变布局', async () => {
  const observers: ResizeObserverCallback[] = []
  class ResizeObserverMock {
    constructor(callback: ResizeObserverCallback) {
      observers.push(callback)
    }
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  const frames = new Map<number, FrameRequestCallback>()
  let frameId = 0
  vi.stubGlobal('ResizeObserver', ResizeObserverMock)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = ++frameId
    frames.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id))

  const wrapper = setup(1, 2)
  const root = wrapper.element as HTMLElement
  let measuredWidth = 120
  let measuredHeight = 80
  Object.defineProperty(root, 'clientWidth', {
    configurable: true,
    get: () => measuredWidth,
  })
  Object.defineProperty(root, 'clientHeight', {
    configurable: true,
    get: () => measuredHeight,
  })
  const canvas = wrapper.get('canvas')
  await nextTick()
  await nextTick()

  expect(canvas.element.style.width).toBe('120px')
  measuredWidth = 321
  measuredHeight = 123
  observers[0]([], {} as ResizeObserver)
  expect(canvas.element.style.width).toBe('120px')
  while (canvas.element.style.width !== '321px') {
    const frame = frames.entries().next().value
    expect(frame).toBeDefined()
    frames.delete(frame![0])
    frame![1](0)
    await nextTick()
  }

  expect(canvas.element.style.width).toBe('321px')
  expect(canvas.element.style.height).toBe('123px')
})

it('表头在字段名下显示数据库类型，缺少类型时明确标注未知', () => {
  const wrapper = setup(1, 2, { columnTypes: ['BIGINT'] })
  const headers = wrapper.findAll('[role="columnheader"]')
  expect(headers[1].text()).toContain('BIGINT')
  expect(headers[1].attributes('title')).toBe('列0\nBIGINT')
  expect(headers[2].text()).toContain('未知')
})

it('鼠标命中、双击和右键沿用原行身份，表头不触发编辑', async () => {
  const wrapper = setup()
  const canvas = wrapper.get('canvas')
  await canvas.trigger('click', { clientX: 200, clientY: 45 })
  expect(wrapper.emitted('select')?.[0]?.[0]).toMatchObject({
    row: { __row: '200' },
    column: { key: 'c1' },
  })
  await canvas.trigger('dblclick', { clientX: 200, clientY: 45 })
  await canvas.trigger('contextmenu', { clientX: 200, clientY: 45 })
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

it('只为当前单元格挂载一个编辑输入，并按列宽计算其内容坐标', async () => {
  const wrapper = setup(20, 5, { editing: { row: 202, column: 2 }, editor: true })
  const editor = wrapper.get<HTMLInputElement>('input[aria-label="编辑中"]')
  expect(wrapper.findAll('input')).toHaveLength(1)
  expect(editor.element.style.left).toBe('324px')
  expect(editor.element.style.top).toBe('90px')
  expect(editor.element.style.width).toBe('140px')

  const columns = wrapper
    .props('columns')
    .map((column, index) => (index === 0 ? { ...column, width: 170 } : column))
  await wrapper.setProps({ columns })
  expect(wrapper.get<HTMLInputElement>('input[aria-label="编辑中"]').element.style.left).toBe(
    '354px'
  )
})

it('编辑框裁切在可视数据区域内，单元格离开视口时结束编辑', async () => {
  const wrapper = setup(20, 5, { editing: { row: 200, column: 0 }, editor: true })
  const root = wrapper.element as HTMLElement
  Object.defineProperty(root, 'clientWidth', { configurable: true, value: 220 })
  Object.defineProperty(root, 'clientHeight', { configurable: true, value: 100 })
  window.dispatchEvent(new Event('resize'))
  await nextTick()

  const editor = wrapper.get<HTMLInputElement>('input[aria-label="编辑中"]')
  expect(editor.element.style.left).toBe('44px')
  expect(editor.element.style.width).toBe('140px')

  root.scrollLeft = 100
  root.dispatchEvent(new Event('scroll'))
  await nextTick()
  expect(editor.element.style.left).toBe('144px')
  expect(editor.element.style.width).toBe('40px')

  root.scrollTop = 1
  root.dispatchEvent(new Event('scroll'))
  await nextTick()
  expect(wrapper.emitted('editor-offscreen')).toHaveLength(1)
})
