import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import Sidebar from './ApiSidebar.vue'
import ContextMenu from '@/core/ui/ContextMenu.vue'
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
  let menu = wrapper.getComponent(ContextMenu)
  menu.props('items').find((item) => item.label === '添加分组')!.onClick!()
  expect(wrapper.emitted('newGroup')?.at(-1)).toEqual([''])
  menu.vm.$emit('close')
  await wrapper.vm.$nextTick()
  const group = wrapper.findAll('button').find((button) => button.text() === '用户1')!
  await group.trigger('contextmenu', { clientX: 20, clientY: 20 })
  menu = wrapper.getComponent(ContextMenu)
  menu.props('items').find((item) => item.label === '创建子分组')!.onClick!()
  expect(wrapper.emitted('newGroup')?.at(-1)).toEqual(['开发/用户'])
  menu.props('items').find((item) => item.label === '新建 WebSocket 接口')!.onClick!()
  expect(wrapper.emitted('new')?.at(-1)).toEqual(['ws', '开发/用户'])
})

it('选中接口的按钮保持透明背景，方法标记显示颜色，点击仍打开接口', async () => {
  const wrapper = mount(Sidebar, {
    props: { apis: [api], groups: ['开发/用户'], activeId: 1, loading: false, error: '' },
  })
  const button = wrapper.findAll('button').find((button) => button.text() === 'GET用户列表')!
  expect(button.classes()).toContain('hover:!bg-transparent')
  expect(button.get('span.font-mono').classes()).toContain('text-success-strong')
  await button.trigger('click')
  expect(wrapper.emitted('select')?.[0]).toEqual([api])
})

it('层级缩进在按钮外层，不受公共按钮内边距影响', () => {
  const wrapper = mount(Sidebar, {
    props: { apis: [api], groups: ['开发/用户'], activeId: 1, loading: false, error: '' },
  })
  const child = wrapper
    .get('[data-api-group-drop="开发/用户"]')
    .element.closest('[data-depth]') as HTMLElement
  expect(child.dataset.depth).toBe('1')
  expect(child.style.paddingLeft).toBe('18px')
  expect(wrapper.find('[data-api-group-drop=""]').exists()).toBe(true)
})

it('拖入分组显示目标并只发出移动；小幅点击、Esc、卸载均不产生移动', async () => {
  const wrapper = mount(Sidebar, {
    attachTo: document.body,
    props: { apis: [api], groups: ['开发/用户', '目标'], activeId: 1, loading: false, error: '' },
  })
  const target = wrapper.get('[data-api-group-drop="目标"]').element
  const hit = vi.spyOn(document, 'elementFromPoint').mockReturnValue(target)
  const source = wrapper.findAll('button').find((button) => button.text() === 'GET用户列表')!
  const pointer = (type: string, x: number) =>
    window.dispatchEvent(
      new PointerEvent(type, { pointerId: 1, button: 0, clientX: x, clientY: 20, cancelable: true })
    )
  await source.trigger('pointerdown', { pointerId: 1, button: 0, clientX: 10, clientY: 20 })
  pointer('pointermove', 12)
  pointer('pointerup', 12)
  expect(wrapper.emitted('move')).toBeUndefined()
  await source.trigger('pointerdown', { pointerId: 1, button: 0, clientX: 10, clientY: 20 })
  pointer('pointermove', 30)
  await wrapper.vm.$nextTick()
  expect(wrapper.get('[data-api-group-drop="目标"]').classes()).toContain('ring-1')
  pointer('pointerup', 30)
  await source.trigger('click')
  expect(wrapper.emitted('move')).toEqual([[1, '目标']])
  expect(wrapper.emitted('select')).toBeUndefined()
  await source.trigger('pointerdown', { pointerId: 1, button: 0, clientX: 10, clientY: 20 })
  pointer('pointermove', 30)
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
  pointer('pointerup', 30)
  expect(wrapper.emitted('move')).toHaveLength(1)
  await source.trigger('pointerdown', { pointerId: 1, button: 0, clientX: 10, clientY: 20 })
  wrapper.unmount()
  const count = hit.mock.calls.length
  pointer('pointermove', 40)
  pointer('pointerup', 40)
  expect(hit).toHaveBeenCalledTimes(count)
})
