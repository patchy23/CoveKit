import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { UiCheckbox, UiSelect, UiTableExpandableRow } from '@/core/ui'
import ExpandableTableShowcase from './ExpandableTableShowcase.vue'

enableAutoUnmount(afterEach)

it('实验室展示子表格、可保留表单与错误重试，不连接业务服务', async () => {
  const wrapper = mount(ExpandableTableShowcase)
  const rows = wrapper.findAllComponents(UiTableExpandableRow)
  expect(wrapper.text()).toContain('orders-web-1')
  await rows[1]!.get('button').trigger('click')
  expect(wrapper.text()).not.toContain('orders-web-1')
  await wrapper.get('[aria-label="演示备注"]').setValue('草稿')
  await rows[1]!.get('button').trigger('click')
  await rows[1]!.get('button').trigger('click')
  expect((wrapper.get('[aria-label="演示备注"]').element as HTMLInputElement).value).toBe('')
  wrapper.findAllComponents(UiCheckbox)[1]!.vm.$emit('update:modelValue', true)
  await wrapper.get('[aria-label="演示备注"]').setValue('保留')
  await rows[1]!.get('button').trigger('click')
  await rows[1]!.get('button').trigger('click')
  expect((wrapper.get('[aria-label="演示备注"]').element as HTMLInputElement).value).toBe('保留')
  wrapper.getComponent(UiSelect).vm.$emit('update:modelValue', 'error')
  await wrapper.vm.$nextTick()
  expect(wrapper.get('[role="alert"]').text()).toContain('读取失败')
  await wrapper.get('[role="alert"] button').trigger('click')
  expect(wrapper.find('[role="alert"]').exists()).toBe(false)
})
