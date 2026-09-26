import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import HostsList from './HostsList.vue'

vi.mock('@/core/ui', () => ({
  UiScrollArea: { template: '<section><slot /></section>' },
  UiButton: { template: '<button><slot /></button>' },
  UiIconButton: { template: '<button>delete</button>' },
  UiIcon: { template: '<i />' },
  UiCheckbox: {
    props: ['modelValue'],
    emits: ['update:modelValue'],
    template:
      '<button @click="$emit(\'update:modelValue\', !modelValue)">{{ modelValue }}</button>',
  },
  UiInput: {
    props: ['modelValue'],
    emits: ['update:modelValue'],
    template:
      '<input :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
  },
}))
enableAutoUnmount(afterEach)

it('复用列表行时 IP、域名、备注、开关与校验仍实时更新，删除不复用错误行', async () => {
  const wrapper = mount(HostsList, {
    props: { content: '# 保留注释\n127.0.0.1 first.test\n0.0.0.0 second.test' },
  })
  const rows = () => wrapper.findAll('section .grid')
  const second = rows()[1]!.element
  const change = () => wrapper.emitted('change')!.at(-1)![0] as string
  await rows()[0]!.findAll('input')[0]!.setValue('999.0.0.0')
  expect(wrapper.text()).toContain('1 处无效')
  expect(change()).toContain('999.0.0.0 first.test')
  await rows()[0]!.findAll('input')[0]!.setValue('127.0.0.2')
  await rows()[0]!.findAll('input')[1]!.setValue('changed.test alias.test')
  await rows()[0]!.findAll('input')[2]!.setValue('中文说明')
  await rows()[0]!.findAll('button')[0]!.trigger('click')
  expect(rows()[0]!.findAll('button')[0]!.text()).toBe('false')
  expect(change()).toContain('# 127.0.0.2 changed.test alias.test # 中文说明')
  expect(rows()[1]!.element).toBe(second)
  await rows()[0]!.findAll('button')[1]!.trigger('click')
  expect(rows()).toHaveLength(1)
  expect(rows()[0]!.element).toBe(second)
  expect(change()).toBe('# 保留注释\n0.0.0.0 second.test')
})
