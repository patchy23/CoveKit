import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { defineComponent } from 'vue'
import UiSelect from './UiSelect.vue'
import UiModal from './UiModal.vue'
import UiField from './UiField.vue'

enableAutoUnmount((cleanup) => {
  afterEach(async () => {
    cleanup()
    await flushPromises()
    document.body.innerHTML = ''
  })
})

it.each(['', '选择认证方式'])('提示为“%s”时，弹窗下拉正常定位、选择并关闭', async (hint) => {
  const wrapper = mount(
    defineComponent({
      components: { UiSelect, UiModal, UiField },
      data: () => ({ group: 'none', auth: 'password', hint, dialogOpen: true }),
      template: `<UiModal :open="dialogOpen" @close="dialogOpen = false" title="添加服务器" description="测试表单">
      <UiField label="分组"><UiSelect v-model="group" :options="[{value:'none',label:'未分组'}]" /></UiField>
      <UiField label="认证方式"><UiSelect v-model="auth" :title="hint" :options="[{value:'password',label:'密码'},{value:'privateKey',label:'私钥'}]" /></UiField>
    </UiModal>`,
    }),
    { attachTo: document.body }
  )
  await flushPromises()
  const groupTrigger = document.querySelectorAll<HTMLElement>('[role="combobox"]')[0]
  groupTrigger.dispatchEvent(
    new PointerEvent('pointerdown', {
      bubbles: true,
      button: 0,
      pointerType: 'mouse',
      pointerId: 1,
    })
  )
  await flushPromises()
  const groupOption = document.querySelector<HTMLElement>('[role="option"]')!
  groupOption.focus()
  groupOption.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' }))
  await flushPromises()
  const trigger = document.querySelectorAll<HTMLElement>('[role="combobox"]')[1]
  trigger.dispatchEvent(
    new PointerEvent('pointerdown', {
      bubbles: true,
      button: 0,
      pointerType: 'mouse',
      pointerId: 1,
    })
  )
  await flushPromises()
  trigger.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true, button: 0 }))
  trigger.dispatchEvent(
    new PointerEvent('pointerup', { bubbles: true, button: 0, pointerType: 'mouse', pointerId: 1 })
  )
  trigger.click()
  await flushPromises()
  expect(trigger.getAttribute('aria-expanded')).toBe('true')
  const floating = document.querySelector<HTMLElement>('[data-reka-popper-content-wrapper]')!
  expect(floating.style.transform).not.toContain('-200%')
  const option = [...document.querySelectorAll<HTMLElement>('[role="option"]')].find((item) =>
    item.textContent?.includes('私钥')
  )!
  expect(option).toBeDefined()
  option.focus()
  option.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' }))
  await flushPromises()
  expect(wrapper.vm.auth).toBe('privateKey')
  document.querySelector<HTMLButtonElement>('button[aria-label="关闭"]')!.click()
  await flushPromises()
  expect(wrapper.vm.dialogOpen).toBe(false)
})

it('禁用态仍传到实际按钮，恢复后可展开', async () => {
  const wrapper = mount(UiSelect, {
    props: {
      modelValue: 'password',
      disabled: true,
      options: [{ value: 'password', label: '密码' }],
    },
    attachTo: document.body,
  })
  expect(wrapper.get('button').attributes('disabled')).toBeDefined()
  await wrapper.get('button').trigger('keydown', { key: 'ArrowDown' })
  expect(document.querySelector('[role="listbox"]')).toBeNull()
  await wrapper.setProps({ disabled: false })
  expect(wrapper.get('button').attributes('disabled')).toBeUndefined()
  await wrapper.get('button').trigger('keydown', { key: 'ArrowDown' })
  await flushPromises()
  expect(wrapper.get('button').attributes('aria-expanded')).toBe('true')
  expect(
    document.querySelector<HTMLElement>('[data-reka-popper-content-wrapper]')!.style.transform
  ).not.toContain('-200%')
})
