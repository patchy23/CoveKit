import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { defineComponent, ref } from 'vue'
import UiContextMenu from './UiContextMenu.vue'
import UiModal from './UiModal.vue'

enableAutoUnmount((cleanup) => {
  afterEach(async () => {
    cleanup()
    await flushPromises()
    vi.restoreAllMocks()
    document.body.innerHTML = ''
  })
})

function menu() {
  const node = document.body.querySelector<HTMLElement>('[role="menu"]')
  if (!node) throw new Error('菜单未挂载')
  return node
}
function key(value: string) {
  document.activeElement?.dispatchEvent(
    new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true })
  )
}

it('重复右键更新原菜单，不在 mousedown 与 contextmenu 之间卸载，左键仍可关闭', async () => {
  const wrapper = mount(
    defineComponent({
      components: { UiContextMenu },
      setup() {
        const open = ref(false)
        const items = ref([{ label: '首次菜单' }])
        function show() {
          items.value = [{ label: open.value ? '更新菜单' : '首次菜单' }]
          open.value = true
        }
        return { open, items, show }
      },
      template:
        '<div><button @contextmenu.stop.prevent="show">目标</button><UiContextMenu v-if="open" :x="10" :y="10" :items="items" @close="open = false" /></div>',
    }),
    { attachTo: document.body }
  )
  const target = wrapper.get('button')
  await target.trigger('contextmenu')
  await flushPromises()
  const original = menu()
  await target.trigger('mousedown', { button: 2 })
  expect(menu()).toBe(original)
  await target.trigger('contextmenu')
  await flushPromises()
  expect(menu()).toBe(original)
  expect(menu().textContent).toContain('更新菜单')
  await target.trigger('mousedown', { button: 0 })
  expect(document.querySelector('[role="menu"]')).toBeNull()
})

it('另一个公共菜单接管时关闭旧菜单，未处理的外部右键也能关闭', async () => {
  const firstClose = vi.fn()
  mount(UiContextMenu, {
    props: { x: 10, y: 10, items: [] },
    attrs: { onClose: firstClose },
    attachTo: document.body,
  })
  await flushPromises()
  const secondClose = vi.fn()
  mount(UiContextMenu, {
    props: { x: 20, y: 20, items: [] },
    attrs: { onClose: secondClose },
    attachTo: document.body,
  })
  await flushPromises()
  expect(firstClose).toHaveBeenCalledTimes(1)
  document.body.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }))
  await flushPromises()
  expect(secondClose).toHaveBeenCalledTimes(1)
})

it('菜单焦点支持方向、首尾和 Escape，跳过禁用项及分隔线', async () => {
  const trigger = document.createElement('button')
  document.body.append(trigger)
  trigger.focus()
  const wrapper = mount(UiContextMenu, {
    props: {
      x: 10,
      y: 10,
      items: [
        { label: '打开' },
        { label: '禁用', disabled: true },
        { label: '', separator: true },
        { label: '删除' },
      ],
    },
    attachTo: document.body,
  })
  await flushPromises()
  expect(document.activeElement?.textContent).toContain('打开')
  key('ArrowDown')
  expect(document.activeElement?.textContent).toContain('删除')
  key('Home')
  expect(document.activeElement?.textContent).toContain('打开')
  key('End')
  expect(document.activeElement?.textContent).toContain('删除')
  key('Escape')
  expect(wrapper.emitted('close')).toHaveLength(1)
  expect(document.activeElement).toBe(trigger)
})

it('菜单能在模态弹窗上取得焦点，选项执行一次后关闭', async () => {
  const modal = mount(UiModal, {
    props: { open: true, title: '凭证', description: '操作菜单测试', closeOnBackdrop: true },
    attachTo: document.body,
  })
  await flushPromises()
  const action = vi.fn()
  const wrapper = mount(UiContextMenu, {
    props: { x: 10, y: 10, items: [{ label: '编辑', onClick: action }] },
    attachTo: document.body,
  })
  await flushPromises()
  const item = menu().querySelector<HTMLButtonElement>('[role="menuitem"]')!
  expect(document.activeElement).toBe(item)
  item.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }))
  await flushPromises()
  expect(modal.emitted('close')).toBeUndefined()
  item.click()
  expect(action).toHaveBeenCalledTimes(1)
  expect(wrapper.emitted('close')).toHaveLength(1)
})

it('菜单根据实际尺寸避让右下角，坐标变化时重新定位', async () => {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    width: 200,
    height: 100,
  } as DOMRect)
  const wrapper = mount(UiContextMenu, {
    props: { x: window.innerWidth - 1, y: window.innerHeight - 1, items: [{ label: '打开' }] },
    attachTo: document.body,
  })
  await flushPromises()
  expect(Number.parseFloat(menu().style.left)).toBe(window.innerWidth - 208)
  expect(Number.parseFloat(menu().style.top)).toBe(window.innerHeight - 108)
  await wrapper.setProps({ x: -20, y: -20 })
  await flushPromises()
  expect(menu().style.left).toBe('8px')
  expect(menu().style.top).toBe('8px')
})

it('空菜单仍能用 Tab 退出，卸载后外部事件不再触发关闭', async () => {
  const close = vi.fn()
  const wrapper = mount(UiContextMenu, {
    props: { x: 0, y: 0, items: [] },
    attrs: { onClose: close },
    attachTo: document.body,
  })
  await flushPromises()
  key('Tab')
  expect(close).toHaveBeenCalledTimes(1)
  wrapper.unmount()
  document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }))
  expect(close).toHaveBeenCalledTimes(1)
})
