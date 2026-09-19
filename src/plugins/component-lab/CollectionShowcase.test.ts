import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import CollectionShowcase from './CollectionShowcase.vue'
import { UiCheckbox, UiSortableList, UiTree } from '@/core/ui'
enableAutoUnmount(afterEach)
afterEach(() => vi.useRealTimers())
it('实验室同时展示树与列表，保存失败可重试且不改变原顺序', async () => {
  vi.useFakeTimers()
  const wrapper = mount(CollectionShowcase)
  const list = wrapper.getComponent(UiSortableList)
  expect(
    wrapper
      .getComponent(UiTree)
      .props('items')
      .some((item) => item.id === 'empty' && item.expandable)
  ).toBe(true)
  wrapper.findAllComponents(UiCheckbox)[1].vm.$emit('update:modelValue', true)
  await wrapper.vm.$nextTick()
  const move = { id: 'verify', targetId: 'prepare', position: 'before' }
  list.vm.$emit('move', move)
  await vi.advanceTimersByTimeAsync(500)
  expect(list.props('items')[0].id).toBe('prepare')
  expect(wrapper.text()).toContain('模拟保存失败')
  wrapper.findAllComponents(UiCheckbox)[1].vm.$emit('update:modelValue', false)
  await wrapper.vm.$nextTick()
  list.vm.$emit('move', move)
  await vi.advanceTimersByTimeAsync(500)
  expect(list.props('items')[0].id).toBe('verify')
})
