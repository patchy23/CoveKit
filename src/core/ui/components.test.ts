import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import UiButton from './UiButton.vue'
import UiInput from './UiInput.vue'
import UiTabs from './UiTabs.vue'

describe('公共 UI 组件', () => {
  it('按钮在加载中自动禁用并显示加载状态', () => {
    const wrapper = mount(UiButton, { props: { loading: true }, slots: { default: '保存' } })
    expect(wrapper.get('button').attributes('disabled')).toBeDefined()
    expect(wrapper.find('.ui-spinner').exists()).toBe(true)
  })

  it('输入框支持 trim 和 number 修饰符', async () => {
    const wrapper = mount(UiInput, { props: { modelModifiers: { trim: true, number: true } } })
    await wrapper.get('input').setValue(' 42 ')
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([42])
  })

  it('页签点击后更新受控值', async () => {
    const wrapper = mount(UiTabs, {
      props: {
        modelValue: 'first',
        items: [
          { value: 'first', label: '第一项' },
          { value: 'second', label: '第二项' },
        ],
      },
    })
    await wrapper.get('button:nth-child(2)').trigger('click')
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['second'])
  })
})
