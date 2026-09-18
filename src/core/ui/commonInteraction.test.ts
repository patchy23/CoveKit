import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { defineComponent, nextTick } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import UiButton from './UiButton.vue'
import UiIconButton from './UiIconButton.vue'
import UiPanel from './UiPanel.vue'
import UiProgress from './UiProgress.vue'
import UiRadioGroup from './UiRadioGroup.vue'

enableAutoUnmount((cleanup) => {
  afterEach(async () => {
    cleanup()
    await flushPromises()
    vi.restoreAllMocks()
    document.body.innerHTML = ''
  })
})

it('裸 disabled 属性和加载中的按钮都不能激活动作', async () => {
  const action = vi.fn()
  const wrapper = mount(
    defineComponent({
      components: { UiButton },
      setup: () => ({ action }),
      template: '<UiButton disabled @click="action">删除</UiButton>',
    }),
    { attachTo: document.body }
  )
  wrapper.get('button').element.click()
  expect(action).not.toHaveBeenCalled()
  const loading = mount(UiButton, { props: { loading: true }, attrs: { onClick: action } })
  expect(loading.get('button').attributes('disabled')).toBeDefined()
  expect(loading.get('button').attributes('aria-busy')).toBe('true')
})

it('链接按钮禁用时无导航入口且不触发监听，恢复后可操作', async () => {
  const action = vi.fn()
  const wrapper = mount(UiButton, {
    props: { as: 'a', disabled: true },
    attrs: { href: '#target', onClick: action },
  })
  expect(wrapper.get('a').attributes('href')).toBeUndefined()
  await wrapper.get('a').trigger('click')
  expect(action).not.toHaveBeenCalled()
  await wrapper.setProps({ disabled: false })
  expect(wrapper.get('a').attributes('href')).toBe('#target')
  await wrapper.get('a').trigger('click')
  expect(action).toHaveBeenCalledTimes(1)
})

it('图标按钮随 size 改变方形尺寸，同时保留禁用透传', async () => {
  const wrapper = mount(UiIconButton, { props: { label: '添加', size: 'xs' } })
  expect(wrapper.get('button').element.style.width).toBe('24px')
  await wrapper.setProps({ size: 'lg' })
  expect(wrapper.get('button').element.style.width).toBe('42px')
  expect(wrapper.get('button').element.style.height).toBe('42px')
  const disabled = mount(UiIconButton, { props: { label: '禁用' }, attrs: { disabled: '' } })
  expect(disabled.get('button').attributes('disabled')).toBeDefined()
})

it('折叠面板支持空格和 Enter，保留内容节点及输入值', async () => {
  const wrapper = mount(UiPanel, {
    props: { collapsible: true, title: '更多配置' },
    slots: { default: '<input value="保留值" />' },
  })
  const input = wrapper.get('input').element
  const trigger = wrapper.get('[role="button"]')
  await trigger.trigger('keydown', { key: ' ' })
  expect(trigger.attributes('aria-expanded')).toBe('false')
  expect(wrapper.get('input').element).toBe(input)
  await trigger.trigger('keydown', { key: 'Enter' })
  expect(trigger.attributes('aria-expanded')).toBe('true')
  expect(input.value).toBe('保留值')
  expect(wrapper.get('input').element.parentElement?.id).toBe(trigger.attributes('aria-controls'))
})

it('互斥筛选使用受控值，禁用选项不提交，方向键跳过禁用项', async () => {
  const wrapper = mount(UiRadioGroup, {
    attachTo: document.body,
    props: {
      modelValue: 'all',
      name: 'filter',
      variant: 'chips',
      options: [
        { value: 'all', label: '全部' },
        { value: 'locked', label: '禁用', disabled: true },
        { value: 'ssh', label: 'SSH' },
      ],
    },
  })
  const items = wrapper.findAll('[role="radio"]')
  await items[1].trigger('click')
  expect(wrapper.emitted('update:modelValue')).toBeUndefined()
  await items[2].trigger('click')
  expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['ssh'])
  await wrapper.setProps({ modelValue: 'ssh' })
  expect(items[2].attributes('aria-checked')).toBe('true')
  ;(items[0].element as HTMLElement).focus()
  await items[0].trigger('keydown', { key: 'ArrowRight' })
  await nextTick()
  expect(document.activeElement).toBe(items[2].element)
})

it('进度越界或非有限数时不输出 NaN，未知进度不宣称百分比', async () => {
  const wrapper = mount(UiProgress, {
    props: { value: 120, max: 100, label: '传输', showValue: true },
  })
  expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('100')
  await wrapper.setProps({ value: Number.NaN, max: 0 })
  expect(wrapper.text()).not.toContain('NaN')
  expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('0')
  await wrapper.setProps({ indeterminate: true })
  expect(wrapper.text()).not.toContain('%')
  expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBeUndefined()
})
