import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import UiButton from './UiButton.vue'
import UiIconButton from './UiIconButton.vue'
import UiTooltip from './UiTooltip.vue'

const wrappers: Array<{ unmount: () => void }> = []
afterEach(() => {
  wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
  vi.restoreAllMocks()
  vi.useRealTimers()
  document.body.innerHTML = ''
})

it('悬停延迟显示纯文本，禁用后关闭且卸载清理浮层', async () => {
  vi.useFakeTimers()
  const wrapper = mount(UiTooltip, {
    attachTo: document.body,
    props: { content: '<b>提示</b>' },
    slots: { default: '<button>操作</button>' },
  })
  wrappers.push(wrapper)
  await wrapper.get('button').trigger('pointermove', { pointerType: 'mouse' })
  await vi.advanceTimersByTimeAsync(399)
  expect(document.querySelector('.ui-tooltip')).toBeNull()
  await vi.advanceTimersByTimeAsync(1)
  await nextTick()
  expect(document.querySelector('.ui-tooltip')?.textContent).toContain('<b>提示</b>')
  expect(document.querySelector('.ui-tooltip b')).toBeNull()
  await wrapper.setProps({ disabled: true })
  expect(document.querySelector('.ui-tooltip')).toBeNull()
  wrapper.unmount()
  wrappers.pop()
  expect(document.querySelector('[role="tooltip"]')).toBeNull()
})

it('按钮保留点击、尺寸、表单类型和无障碍标签，不输出原生 title', async () => {
  const click = vi.fn()
  const wrapper = mount(UiIconButton, {
    attachTo: document.body,
    props: { label: '添加连接', size: 'xs' },
    attrs: { onClick: click, title: '新建 SSH 连接', type: 'submit' },
  })
  wrappers.push(wrapper)
  const button = wrapper.get('button')
  expect(wrapper.findAll('button')).toHaveLength(1)
  expect(button.attributes('title')).toBeUndefined()
  expect(button.attributes('aria-label')).toBe('添加连接')
  expect(button.attributes('type')).toBe('submit')
  expect(button.classes()).toContain('ui-control-xs')
  await button.trigger('click')
  expect(click).toHaveBeenCalledTimes(1)
})

it('键盘聚焦显示提示并建立描述关联，Esc 关闭', async () => {
  const wrapper = mount(UiButton, {
    attachTo: document.body,
    props: { title: '保存修改' },
    slots: { default: '保存' },
  })
  wrappers.push(wrapper)
  const button = wrapper.get('button')
  vi.spyOn(button.element, 'matches').mockImplementation(
    (selector) => selector === ':focus-visible'
  )
  await button.trigger('focus')
  const description = button.attributes('aria-describedby')
  expect(description).toBeTruthy()
  expect(document.getElementById(description)?.textContent).toContain('保存修改')
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  await nextTick()
  expect(document.querySelector('.ui-tooltip')).toBeNull()
})

it('空提示保持单一按钮且不渲染浮层', () => {
  const wrapper = mount(UiButton, { props: { loading: true }, slots: { default: '保存' } })
  wrappers.push(wrapper)
  expect(wrapper.findAll('button')).toHaveLength(1)
  expect(wrapper.get('button').attributes('disabled')).toBeDefined()
  expect(wrapper.find('.ui-spinner').exists()).toBe(true)
  expect(wrapper.find('.ui-tooltip').exists()).toBe(false)
})
