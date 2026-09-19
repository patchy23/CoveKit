import { afterEach, expect, it } from 'vitest'
import { enableAutoUnmount, mount } from '@vue/test-utils'
import Showcase from './SelectionDialogShowcase.vue'
import { UiSlider, UiKeyValueEditor, UiTreeSelect, UiConfirmDialog, UiInputDialog } from '@/core/ui'
enableAutoUnmount(afterEach)
it('统一检查页使用新公共入口并显示确认与输入的操作反馈', async () => {
  const wrapper = mount(Showcase)
  expect(wrapper.findComponent(UiSlider).exists()).toBe(true)
  expect(wrapper.findComponent(UiKeyValueEditor).exists()).toBe(true)
  expect(wrapper.findComponent(UiTreeSelect).exists()).toBe(true)
  const confirm = wrapper
    .findAll('button')
    .find((button) => button.text().includes('UiConfirmDialog'))!
  await confirm.trigger('click')
  expect(wrapper.getComponent(UiConfirmDialog).props('open')).toBe(true)
  wrapper.getComponent(UiConfirmDialog).vm.$emit('confirm')
  await wrapper.vm.$nextTick()
  expect(wrapper.get('[role="status"]').text()).toBe('已确认')
  const input = wrapper.findAll('button').find((button) => button.text().includes('UiInputDialog'))!
  await input.trigger('click')
  wrapper.getComponent(UiInputDialog).vm.$emit('confirm', '检查完成')
  await wrapper.vm.$nextTick()
  expect(wrapper.get('[role="status"]').text()).toBe('输入：检查完成')
})
