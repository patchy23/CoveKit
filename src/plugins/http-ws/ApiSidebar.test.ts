import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import Sidebar from './ApiSidebar.vue'
import { UiTree, UiSortableList } from '@/core/ui'
import { UiContextMenu } from '@/core/ui'
import type { ApiRecord } from './contracts'
enableAutoUnmount(afterEach)
afterEach(() => vi.restoreAllMocks())
const api: ApiRecord = {
  id: 1,
  name: '用户列表',
  type: 'http',
  method: 'GET',
  url: 'https://example.invalid',
  groupName: '开发/用户',
  params: '[]',
  headers: '[]',
  bodyMode: 'none',
  body: '',
  options: '{}',
  updatedAt: '',
}

it('空白右键添加分组，分组右键创建子分组或绑定该组的新接口', async () => {
  const wrapper = mount(Sidebar, {
    attachTo: document.body,
    props: { apis: [api], groups: ['开发/用户', '空分组'], activeId: 1, loading: false, error: '' },
  })
  await wrapper.get('aside').trigger('contextmenu', { clientX: 10, clientY: 10 })
  let menu = wrapper.getComponent(UiContextMenu)
  menu.props('items').find((item) => item.label === '添加分组')!.onClick!()
  expect(wrapper.emitted('newGroup')?.at(-1)).toEqual([''])
  menu.vm.$emit('close')
  await wrapper.vm.$nextTick()
  const group = wrapper.get('[data-collection-id="group:开发/用户"]')
  await group.trigger('contextmenu', { clientX: 20, clientY: 20 })
  menu = wrapper.getComponent(UiContextMenu)
  menu.props('items').find((item) => item.label === '创建子分组')!.onClick!()
  expect(wrapper.emitted('newGroup')?.at(-1)).toEqual(['开发/用户'])
  menu.props('items').find((item) => item.label === '新建 WebSocket 接口')!.onClick!()
  expect(wrapper.emitted('new')?.at(-1)).toEqual(['ws', '开发/用户'])
  menu.props('items').find((item) => item.label === '移动到…')!.onClick!()
  expect(wrapper.emitted('requestMoveGroup')?.at(-1)).toEqual(['开发/用户'])
  menu.vm.$emit('close')
  await wrapper.vm.$nextTick()
  await wrapper.get('[data-collection-id="api:1"]').trigger('contextmenu')
  menu = wrapper.getComponent(UiContextMenu)
  menu.props('items').find((item) => item.label === '移动到…')!.onClick!()
  expect(wrapper.emitted('requestMove')?.at(-1)).toEqual([api])
  expect(menu.props('items').some((item) => item.label === '重命名')).toBe(true)
})

it('选中整行、方法标记显示颜色，点击仍打开接口', async () => {
  const wrapper = mount(Sidebar, {
    props: { apis: [api], groups: ['开发/用户'], activeId: 1, loading: false, error: '' },
  })
  const button = wrapper.get('[data-collection-id="api:1"]')
  expect(button.attributes('aria-selected')).toBe('true')
  expect(button.get('span.font-mono').classes()).toContain('text-success-strong')
  await button.trigger('click')
  expect(wrapper.emitted('select')?.[0]).toEqual([api])
})

it('层级缩进在按钮外层，不受公共按钮内边距影响', () => {
  const wrapper = mount(Sidebar, {
    props: { apis: [api], groups: ['开发/用户'], activeId: 1, loading: false, error: '' },
  })
  const child = wrapper
    .get('[data-collection-id="group:开发/用户"]')
    .element.closest('[data-depth]') as HTMLElement
  expect(child.dataset.depth).toBe('1')
  expect(child.style.paddingLeft).toBe('23px')
  expect(wrapper.find('[data-collection-id="group:"]').exists()).toBe(true)
})

it('搜索使用公共列表且禁止拖动，目录移动由公共树发出请求', async () => {
  const wrapper = mount(Sidebar, {
    props: { apis: [api], groups: ['开发/用户'], activeId: 1, loading: false, error: '' },
  })
  const tree = wrapper.getComponent(UiTree)
  const move = { id: 'api:1', targetId: 'group:', position: 'inside' as const }
  tree.vm.$emit('move', move)
  expect(wrapper.emitted('treeMove')).toEqual([[move]])
  await wrapper.get('input').setValue('用户')
  expect(wrapper.findComponent(UiTree).exists()).toBe(false)
  expect(wrapper.getComponent(UiSortableList).props('filtered')).toBe(true)
})
