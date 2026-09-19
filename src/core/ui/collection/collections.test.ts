import { enableAutoUnmount, mount, type VueWrapper } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import UiTree from '../UiTree.vue'
import UiSortableList from '../UiSortableList.vue'
import UiIcon from '../UiIcon.vue'
import { validMove } from './types'
enableAutoUnmount(afterEach)
afterEach(() => {
  vi.restoreAllMocks()
  vi.useRealTimers()
})
const rows = [
  { id: 'a', label: '目录一', depth: 0, expandable: true, expanded: true },
  { id: 'child', label: '子项', depth: 1 },
  { id: 'empty', label: '空目录', depth: 0, expandable: true, expanded: false },
  { id: 'locked', label: '不可用', depth: 0, disabled: true },
]
it('叶子默认无图标，分支保留文件夹且业务可显式提供叶子图标', () => {
  const tree = mount(UiTree, { props: { items: rows } })
  expect(tree.get('[data-collection-id="child"]').findComponent(UiIcon).exists()).toBe(false)
  expect(
    tree
      .get('[data-collection-id="a"]')
      .findAllComponents(UiIcon)
      .map((icon: VueWrapper) => (icon.props() as { name: string }).name)
  ).toContain('folder')
  const custom = mount(UiTree, {
    props: { items: rows },
    slots: { icon: '<span data-business-icon>GET</span>' },
  })
  expect(custom.get('[data-collection-id="child"] [data-business-icon]').text()).toBe('GET')
  const list = mount(UiSortableList, {
    props: { items: [{ id: 'one', label: '项目' }], draggable: false },
  })
  expect(list.findComponent(UiIcon).exists()).toBe(false)
})
const pointer = (type: string, x = 30, y = 15) =>
  window.dispatchEvent(
    new PointerEvent(type, { pointerId: 1, button: 0, clientX: x, clientY: y, cancelable: true })
  )
function rect(element: Element, top = 0) {
  vi.spyOn(element, 'getBoundingClientRect').mockReturnValue({
    top,
    bottom: top + 28,
    height: 28,
    left: 0,
    right: 180,
    width: 180,
    x: 0,
    y: top,
    toJSON: () => ({}),
  })
}

it('多棵树有独立焦点和 DOM 身份；叶子的右方向键不跳到兄弟节点', async () => {
  const wrapper = mount(
    {
      components: { UiTree },
      setup: () => ({ rows }),
      template: '<UiTree :items="rows" /><UiTree :items="rows" />',
    },
    { attachTo: document.body }
  )
  const [a, b] = wrapper.findAllComponents(UiTree)
  expect(a.get('[role="treeitem"]').attributes('tabindex')).toBe('0')
  expect(a.get('[role="treeitem"]').attributes('id')).not.toBe(
    b.get('[role="treeitem"]').attributes('id')
  )
  const source = a.get('[data-collection-id="child"]')
  ;(source.element as HTMLElement).focus()
  await source.trigger('keydown', { key: 'ArrowRight' })
  expect(document.activeElement).toBe(source.element)
  await source.trigger('keydown', { key: 'Enter', ctrlKey: true })
  expect(a.emitted('open')?.[0]?.[0]).toMatchObject({ id: 'child' })
  await source.trigger('keydown', { key: 'ArrowLeft' })
  expect(document.activeElement).toBe(a.get('[data-collection-id="a"]').element)
})

it('拖动只显示覆盖式落点，列表节点和源顺序保持不动，松开只发出一次请求', async () => {
  const wrapper = mount(UiTree, {
    attachTo: document.body,
    props: { items: rows, draggable: true },
  })
  const source = wrapper.get('[data-collection-id="child"]')
  const target = wrapper.get('[data-collection-id="empty"]')
  rect(target.element)
  vi.spyOn(document, 'elementFromPoint').mockReturnValue(target.element)
  await source.trigger('pointerdown', { pointerId: 1, button: 0, clientX: 0, clientY: 15 })
  pointer('pointermove')
  await wrapper.vm.$nextTick()
  expect(wrapper.findAll('[role="treeitem"]')).toHaveLength(4)
  expect(wrapper.text()).not.toContain('移至根目录')
  expect(target.classes()).toContain('ring-1')
  pointer('pointerup')
  await source.trigger('click')
  expect(wrapper.emitted('move')).toEqual([
    [{ id: 'child', targetId: 'empty', position: 'inside' }],
  ])
  expect(wrapper.emitted('select')).toBeUndefined()
  expect(wrapper.props('items')).toEqual(rows)
})

it('小幅移动、Esc、失焦和卸载不提交；忙碌和搜索态不允许移动', async () => {
  const wrapper = mount(UiSortableList, { attachTo: document.body, props: { items: rows } })
  const source = wrapper.get('[data-collection-id="a"]')
  const target = wrapper.get('[data-collection-id="empty"]')
  rect(target.element)
  const hit = vi.spyOn(document, 'elementFromPoint').mockReturnValue(target.element)
  const start = () =>
    source.trigger('pointerdown', { pointerId: 1, button: 0, clientX: 0, clientY: 15 })
  await start()
  pointer('pointermove', 2)
  pointer('pointerup', 2)
  expect(wrapper.emitted('move')).toBeUndefined()
  for (const event of [new KeyboardEvent('keydown', { key: 'Escape' }), new Event('blur')]) {
    await start()
    pointer('pointermove')
    window.dispatchEvent(event)
    pointer('pointerup')
  }
  await wrapper.setProps({ busy: true })
  await start()
  pointer('pointermove')
  pointer('pointerup')
  await wrapper.setProps({ busy: false, filtered: true })
  await start()
  pointer('pointermove')
  pointer('pointerup')
  expect(wrapper.emitted('move')).toBeUndefined()
  await wrapper.setProps({ filtered: false })
  await start()
  wrapper.unmount()
  const calls = hit.mock.calls.length
  pointer('pointermove')
  pointer('pointerup')
  expect(hit.mock.calls.length).toBe(calls)
})

it('键盘排序与业务禁放规则共用校验；嵌入操作按钮不选中或拖动行', async () => {
  const wrapper = mount(UiSortableList, {
    props: { items: rows, canDrop: (move) => move.targetId !== 'empty' },
    slots: { suffix: '<button type="button">操作</button>' },
  })
  const source = wrapper.get('[data-collection-id="child"]')
  await source.trigger('keydown', { key: 'ArrowUp', altKey: true })
  expect(wrapper.emitted('move')?.[0]).toEqual([{ id: 'child', targetId: 'a', position: 'before' }])
  await source.trigger('keydown', { key: 'ArrowDown', altKey: true })
  expect(wrapper.emitted('move')).toHaveLength(1)
  await source.get('button').trigger('click')
  expect(wrapper.emitted('select')).toBeUndefined()
  expect(validMove(rows, { id: 'a', targetId: 'child', position: 'after' }, true)).toBe(false)
  expect(validMove(rows, { id: 'a', targetId: 'locked', position: 'inside' }, true)).toBe(false)
})

it('悬停展开与边缘滚动无需持续移动鼠标，取消后停止计时', async () => {
  vi.useFakeTimers()
  const wrapper = mount(UiTree, {
    attachTo: document.body,
    props: { items: rows, draggable: true },
  })
  const source = wrapper.get('[data-collection-id="child"]')
  const target = wrapper.get('[data-collection-id="empty"]')
  const root = wrapper.get('[role="tree"]')
  rect(root.element)
  rect(target.element)
  vi.spyOn(document, 'elementFromPoint').mockReturnValue(target.element)
  await source.trigger('pointerdown', { pointerId: 1, button: 0, clientX: 0, clientY: 15 })
  pointer('pointermove', 30, 16)
  await vi.advanceTimersByTimeAsync(750)
  expect(wrapper.emitted('toggle')?.[0]?.[0]).toMatchObject({ id: 'empty' })
  window.dispatchEvent(new Event('blur'))
  const count = wrapper.emitted('toggle')?.length
  await vi.advanceTimersByTimeAsync(1000)
  expect(wrapper.emitted('toggle')).toHaveLength(count!)
})
