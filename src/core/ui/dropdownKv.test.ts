import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { describe, expect, it } from 'vitest'
import UiDropdownMenu from './UiDropdownMenu.vue'
import UiKeyValueEditor from './UiKeyValueEditor.vue'

describe('UiDropdownMenu', () => {
  it('触发器打开菜单，选中项 emit select 与 update:modelValue', async () => {
    const wrapper = mount(UiDropdownMenu, {
      attachTo: document.body,
      props: {
        items: [
          { value: 'a', label: '选项 A' },
          { value: 'b', label: '选项 B' },
        ],
      },
    })
    await wrapper.get('button').trigger('click')
    await nextTick()
    const items = document.body.querySelectorAll<HTMLElement>('[role="menuitem"]')
    expect(items.length).toBe(2)
    items[1].click()
    await nextTick()
    expect(wrapper.emitted('select')?.[0]).toEqual(['b'])
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['b'])
    wrapper.unmount()
  })

  it('禁用项不可选中', async () => {
    const wrapper = mount(UiDropdownMenu, {
      attachTo: document.body,
      props: { items: [{ value: 'a', label: '选项 A', disabled: true }] },
    })
    await wrapper.get('button').trigger('click')
    await nextTick()
    const item = document.body.querySelector<HTMLElement>('[role="menuitem"]')
    expect(item?.getAttribute('aria-disabled')).toBe('true')
    item!.click()
    await nextTick()
    expect(wrapper.emitted('select')).toBeUndefined()
    wrapper.unmount()
  })
})

describe('UiKeyValueEditor', () => {
  const rows = [{ id: 'r1', key: 'Accept', value: 'application/json' }]

  it('编辑键/值 emit 更新后的行数组', async () => {
    const wrapper = mount(UiKeyValueEditor, { props: { rows } })
    const inputs = wrapper.findAll('input')
    await inputs[0].setValue('Content-Type')
    expect(wrapper.emitted('update:rows')?.[0]).toEqual([
      [{ id: 'r1', key: 'Content-Type', value: 'application/json' }],
    ])
  })

  it('添加行生成新 id，删除行按 id 过滤', async () => {
    const wrapper = mount(UiKeyValueEditor, { props: { rows } })
    const buttons = wrapper.findAll('button')
    await buttons[buttons.length - 1].trigger('click')
    const added = wrapper.emitted('update:rows')?.[0]?.[0] as { id: string }[]
    expect(added.length).toBe(2)
    expect(added[1].id).toBeTruthy()
    expect(added[1].id).not.toBe('r1')

    await wrapper.findAll('button')[0].trigger('click')
    expect(wrapper.emitted('update:rows')?.[1]).toEqual([[]])
  })
})
