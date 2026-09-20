import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import UiLogViewer from './UiLogViewer.vue'
import UiSelect from './UiSelect.vue'

enableAutoUnmount(afterEach)
function setup(content = 'before') {
  return mount(UiLogViewer, { props: { content }, attachTo: document.body })
}
function button(wrapper: ReturnType<typeof setup>, text: string) {
  return wrapper.findAll('button').find((item) => item.text() === text)!
}
it('暂停冻结文本节点、选区与滚动位置；恢复使用最新快照，不回放中间内容', async () => {
  const wrapper = setup('first line\nsecond line')
  await button(wrapper, '暂停显示').trigger('click')
  const pre = wrapper.get('pre').element
  const node = pre.firstChild!
  const selection = window.getSelection()!
  const range = document.createRange()
  range.setStart(node, 0)
  range.setEnd(node, 5)
  selection.removeAllRanges()
  selection.addRange(range)
  document.dispatchEvent(new Event('selectionchange'))
  pre.scrollTop = 20
  await wrapper.setProps({ content: 'middle' })
  await wrapper.setProps({ content: 'latest' })
  expect(pre.firstChild).toBe(node)
  expect(selection.toString()).toBe('first')
  expect(pre.scrollTop).toBe(20)
  await button(wrapper, '复制选中').trigger('click')
  await button(wrapper, '复制全部').trigger('click')
  expect(wrapper.emitted('copy')).toEqual([['first'], ['first line\nsecond line']])
  await button(wrapper, '继续显示').trigger('click')
  expect(pre.textContent).toBe('latest')
  selection.removeAllRanges()
})
it('统一上限截断显示，暂停时改变上限仍保持快照，恢复采用新上限', async () => {
  const wrapper = setup(Array.from({ length: 2100 }, (_, i) => `line-${i}`).join('\n'))
  expect(wrapper.get('pre').text().split('\n')).toHaveLength(300)
  await button(wrapper, '暂停显示').trigger('click')
  wrapper.getComponent(UiSelect).vm.$emit('update:modelValue', '100')
  await wrapper.vm.$nextTick()
  expect(wrapper.emitted('limit-change')).toEqual([[100]])
  expect(wrapper.get('pre').text().split('\n')).toHaveLength(300)
  await button(wrapper, '继续显示').trigger('click')
  expect(wrapper.get('pre').text().split('\n')).toHaveLength(100)
  expect(wrapper.get('pre').text()).toContain('line-2099')
})
