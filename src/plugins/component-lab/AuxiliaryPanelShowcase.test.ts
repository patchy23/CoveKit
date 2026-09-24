import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import AuxiliaryPanelShowcase from './AuxiliaryPanelShowcase.vue'
import { UiBottomPanel, UiPopover } from '@/core/ui'
enableAutoUnmount(afterEach)
vi.stubGlobal(
  'ResizeObserver',
  class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
)
it('底部面板收起再打开保留输入，键盘可调整高度', async () => {
  const wrapper = mount(AuxiliaryPanelShowcase, { attachTo: document.body })
  const panel = wrapper.getComponent(UiBottomPanel)
  const input = panel.get('input')
  await input.setValue('保留输入')
  await panel.get('[role="separator"]').trigger('keydown', { key: 'ArrowUp' })
  expect(panel.attributes('style')).toContain('276px')
  panel.vm.$emit('update:open', false)
  await flushPromises()
  expect(panel.isVisible()).toBe(false)
  await wrapper
    .findAll('button')
    .find((b) => b.text().includes('展开底部'))!
    .trigger('click')
  expect(panel.get('input').element).toBe(input.element)
  expect((input.element as HTMLInputElement).value).toBe('保留输入')
})
it('点击弹层使用 Portal，外部关闭后可以再次打开', async () => {
  const wrapper = mount(AuxiliaryPanelShowcase, { attachTo: document.body })
  const popover = wrapper.getComponent(UiPopover)
  await wrapper
    .findAll('button')
    .find((b) => b.text() === '打开弹层')!
    .trigger('click')
  await flushPromises()
  expect(document.querySelector('[role="dialog"][aria-label="可交互弹层"]')).not.toBeNull()
  popover.vm.$emit('update:open', false)
  await flushPromises()
  expect(document.querySelector('[role="dialog"][aria-label="可交互弹层"]')).toBeNull()
})

it('窗口最小化不卸载输入内容，恢复后可继续编辑', async () => {
  const wrapper = mount(AuxiliaryPanelShowcase, { attachTo: document.body })
  const panel = wrapper.findAllComponents({ name: 'UiFloatingWindow' })[0]
  const input = panel.get('input')
  await input.setValue('草稿仍然存在')
  await panel.get('[aria-label="最小化窗口"]').trigger('click')
  expect(panel.isVisible()).toBe(false)
  await wrapper
    .findAll('button')
    .find((b) => b.text() === '恢复窗口')!
    .trigger('click')
  expect(panel.isVisible()).toBe(true)
  expect(panel.get('input').element).toBe(input.element)
  expect((input.element as HTMLInputElement).value).toBe('草稿仍然存在')
})
