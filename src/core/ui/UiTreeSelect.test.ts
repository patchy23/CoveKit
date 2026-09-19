import { afterEach, expect, it } from 'vitest'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { defineComponent, ref } from 'vue'
import UiTreeSelect from './UiTreeSelect.vue'
import UiModal from './UiModal.vue'
import { treeSelectRows } from './treeSelect'
enableAutoUnmount(afterEach)
const options = [
  {
    value: 'dev',
    label: '开发',
    children: [
      { value: 'users', label: '用户' },
      { value: 'blocked', label: '禁用', disabled: true },
    ],
  },
  { value: 'prod', label: '生产' },
]
it('搜索保留祖先，折叠不丢完整路径，禁用状态保留', () => {
  const closed = treeSelectRows(options, new Set())
  expect(closed.rows.map((row) => row.id)).toEqual(['dev', 'prod'])
  expect(closed.labels.get('users')).toBe('开发 / 用户')
  expect(treeSelectRows(options, new Set(), '用户').rows.map((row) => row.id)).toEqual([
    'dev',
    'users',
  ])
  expect(
    treeSelectRows(options, new Set(['dev'])).rows.find((row) => row.id === 'blocked')?.disabled
  ).toBe(true)
})
it('弹窗内搜索并用键盘选择，回显路径；Esc 只关闭下拉', async () => {
  const wrapper = mount(
    defineComponent({
      components: { UiTreeSelect, UiModal },
      setup() {
        return { value: ref('prod'), dialog: ref(true), options }
      },
      template:
        '<UiModal :open="dialog" title="移动" @close="dialog = false"><UiTreeSelect v-model="value" :options="options" label="目标" /></UiModal>',
    }),
    { attachTo: document.body }
  )
  await flushPromises()
  const trigger = document.querySelector<HTMLButtonElement>('button[aria-label="目标"]')!
  trigger.click()
  await flushPromises()
  const input = document.querySelector<HTMLInputElement>(
    '[data-reka-popper-content-wrapper] input'
  )!
  expect(input).toBeTruthy()
  expect(input.getAttribute('aria-label')).toBe('搜索目标')
  const floating = document.querySelector<HTMLElement>('[data-reka-popper-content-wrapper]')!
  expect(floating.style.transform).not.toContain('-200%')
  input.value = '用户'
  input.dispatchEvent(new Event('input', { bubbles: true }))
  await flushPromises()
  input.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowDown' }))
  expect(document.activeElement?.getAttribute('role')).toBe('treeitem')
  const user = [...document.querySelectorAll<HTMLElement>('[role="treeitem"]')].find((el) =>
    el.textContent?.includes('用户')
  )!
  user.focus()
  user.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' }))
  await flushPromises()
  expect(wrapper.findComponent(UiTreeSelect).props('modelValue')).toBe('users')
  expect(trigger.textContent).toContain('开发 / 用户')
  trigger.click()
  await flushPromises()
  document
    .querySelector<HTMLInputElement>('[data-reka-popper-content-wrapper] input')!
    .dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }))
  await flushPromises()
  expect(trigger.getAttribute('aria-expanded')).toBe('false')
  expect(wrapper.findComponent(UiModal).props('open')).toBe(true)
})
