import { enableAutoUnmount, mount } from '@vue/test-utils'
import { defineComponent, h, onMounted, onUnmounted } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import UiTableExpandableRow from './UiTableExpandableRow.vue'

enableAutoUnmount(afterEach)

it('展开由消费方控制，按钮关联跨列区域，禁用时不触发', async () => {
  const wrapper = mount(UiTableExpandableRow, {
    props: { expanded: false, columns: 4, label: '项目' },
    slots: { details: '详情' },
  })
  const button = wrapper.get('button')
  expect(button.attributes('aria-expanded')).toBe('false')
  expect(wrapper.find('[role="region"]').exists()).toBe(false)
  await button.trigger('click')
  expect(wrapper.emitted('update:expanded')).toEqual([[true]])
  expect(wrapper.find('[role="region"]').exists()).toBe(false)
  await wrapper.setProps({ expanded: true })
  expect(button.attributes('aria-expanded')).toBe('true')
  expect(wrapper.get('[role="region"]').attributes('id')).toBe(button.attributes('aria-controls'))
  expect(wrapper.get('[colspan]').attributes('colspan')).toBe('4')
  await wrapper.setProps({ disabled: true })
  await button.trigger('click')
  expect(wrapper.emitted('update:expanded')).toHaveLength(1)
})

it.each([false, true])('按首次展开懒挂载，keepMounted=%s 时按约定释放资源', async (keepMounted) => {
  const start = vi.fn(),
    stop = vi.fn()
  const child = defineComponent({
    setup() {
      onMounted(start)
      onUnmounted(stop)
      return () => h('span', '内容')
    },
  })
  const wrapper = mount(UiTableExpandableRow, {
    props: { expanded: false, columns: 2, label: '项目', keepMounted },
    slots: { details: () => h(child) },
  })
  expect(start).not.toHaveBeenCalled()
  await wrapper.setProps({ expanded: true })
  expect(start).toHaveBeenCalledTimes(1)
  await wrapper.setProps({ expanded: false })
  expect(stop).toHaveBeenCalledTimes(keepMounted ? 0 : 1)
  await wrapper.setProps({ expanded: true })
  expect(start).toHaveBeenCalledTimes(keepMounted ? 1 : 2)
  wrapper.unmount()
  expect(stop).toHaveBeenCalledTimes(keepMounted ? 1 : 2)
})
