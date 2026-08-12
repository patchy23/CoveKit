import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import UiButton from './UiButton.vue'
import UiInput from './UiInput.vue'
import UiTabs from './UiTabs.vue'
import UiCheckbox from './UiCheckbox.vue'
import UiPagination from './UiPagination.vue'
import UiSelect from './UiSelect.vue'
import UiSwitch from './UiSwitch.vue'

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
    const tabs = wrapper.findAll('[role="tab"]')
    await tabs[1].trigger('mousedown', { button: 0, ctrlKey: false })
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['second'])
  })

  it('基础控件输出统一尺寸类', () => {
    expect(mount(UiButton, { props: { size: 'xs' } }).classes()).toContain('ui-control-xs')
    expect(
      mount(UiInput, { props: { size: 'lg' } })
        .get('input')
        .classes()
    ).toContain('ui-control-lg')
    expect(
      mount(UiSelect, {
        props: { modelValue: 'a', options: [{ value: 'a' }], size: 'sm' },
      })
        .get('button')
        .classes()
    ).toContain('ui-control-sm')
  })

  it('复选框和开关保持受控更新', async () => {
    const checkbox = mount(UiCheckbox, { props: { modelValue: false } })
    await checkbox.get('[role="checkbox"]').trigger('click')
    expect(checkbox.emitted('update:modelValue')?.[0]).toEqual([true])

    const toggle = mount(UiSwitch, { props: { modelValue: false } })
    await toggle.get('button').trigger('click')
    expect(toggle.emitted('update:modelValue')?.[0]).toEqual([true])
  })

  it('分页不会越过首尾页', async () => {
    const wrapper = mount(UiPagination, { props: { modelValue: 1, totalPages: 3 } })
    const buttons = wrapper.findAll('button')
    expect(buttons[0].attributes('disabled')).toBeDefined()
    await buttons[buttons.length - 1].trigger('click')
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([2])
  })
})
