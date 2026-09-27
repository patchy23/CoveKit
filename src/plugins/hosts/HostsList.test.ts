import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import HostsList from './HostsList.vue'
import { entryToText } from './useHosts'

vi.mock('./useHosts', async (original) => {
  const actual = await original<typeof import('./useHosts')>()
  return { ...actual, entryToText: vi.fn(actual.entryToText) }
})

vi.mock('@/core/ui', async () => ({
  useRowWindow: (await import('@/core/ui/collection/useRowWindow')).useRowWindow,
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
afterEach(() => vi.clearAllMocks())

it('连续编辑只格式化修改行，增删同步完整正文且保留空行和纯注释', async () => {
  const wrapper = mount(HostsList, {
    props: { content: '# 原样\r\n\n127.0.0.1 a.test\n0.0.0.0 b.test\n' },
  })
  // 首次编辑按原路径建立完整正文，之后仅格式化变化行。
  await wrapper.findAll('section .grid')[1]!.findAll('input')[1]!.setValue('first-edit.test')
  vi.mocked(entryToText).mockClear()
  const rows = () => wrapper.findAll('section .grid')
  const change = () => wrapper.emitted('change')!.at(-1)![0] as string
  await rows()[1]!.findAll('input')[1]!.setValue('changed.test')
  expect(entryToText).toHaveBeenCalledTimes(1)
  expect(change()).toBe('# 原样\r\n\n127.0.0.1 a.test\n0.0.0.0 changed.test\n')
  await rows()[0]!.findAll('button')[1]!.trigger('click')
  expect(entryToText).toHaveBeenCalledTimes(1)
  expect(change()).toBe('# 原样\r\n\n0.0.0.0 changed.test\n')
  const add = wrapper.findAll('button').find((button) => button.text().includes('新增映射'))!
  await add.trigger('click')
  await add.trigger('click')
  expect(rows()).toHaveLength(3)
  await rows()[1]!.findAll('input')[0]!.setValue('127.0.0.2')
  await rows()[1]!.findAll('input')[1]!.setValue('new.test')
  expect(change()).toBe('# 原样\r\n\n0.0.0.0 changed.test\n\n127.0.0.2 new.test\n')
  await rows()[2]!.findAll('button')[1]!.trigger('click')
  expect(change()).toBe('# 原样\r\n\n0.0.0.0 changed.test\n\n127.0.0.2 new.test')
})

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

it('大列表回收离屏节点，保留焦点及组合输入，跨行 Tab 从完整模型挂载目标', async () => {
  const wrapper = mount(HostsList, {
    attachTo: document.body,
    props: {
      content: Array.from({ length: 10000 }, (_, i) => `127.0.0.1 host${i}.test`).join('\n'),
    },
  })
  const root = wrapper.find('section > div')
  const rows = () => wrapper.findAll('[data-host-entry]')
  expect(rows().length).toBeLessThan(100)
  const first = rows()[0]!
  const original = first.element
  const input = first.findAll('input')[1]!
  ;(input.element as HTMLInputElement).focus()
  await input.trigger('compositionstart')
  ;(root.element as HTMLElement).scrollTop = 62 * 5000
  await root.trigger('scroll')
  expect(wrapper.find(`[data-host-entry="${first.attributes('data-host-entry')}"]`).element).toBe(
    original
  )
  await input.setValue('still-editable.test')
  expect((wrapper.emitted('change')!.at(-1)![0] as string).split('\n')).toHaveLength(10000)
  expect(wrapper.emitted('change')!.at(-1)![0]).toContain('still-editable.test')
  await input.trigger('compositionend')
  const lastButton = first.findAll('button').at(-1)!
  ;(lastButton.element as HTMLButtonElement).focus()
  await lastButton.trigger('keydown', { key: 'Tab' })
  expect(
    document.activeElement?.closest('[data-host-entry]')?.getAttribute('data-host-entry')
  ).toBe('l1')
  await wrapper
    .find('[data-host-entry="l1"] button')
    .trigger('keydown', { key: 'Tab', shiftKey: true })
  expect(document.activeElement).toBe(lastButton.element)
  ;(document.activeElement as HTMLElement).blur()
  ;(root.element as HTMLElement).scrollTop = 62 * 9000
  await root.trigger('scroll')
  expect(wrapper.find('[data-host-entry="l0"]').exists()).toBe(false)
  expect(rows().length).toBeLessThan(100)
})

it('组合输入结束前不会因失焦回收节点，结束后释放；删除锚点前的行保持同一可见条目', async () => {
  const wrapper = mount(HostsList, {
    attachTo: document.body,
    props: {
      content: Array.from({ length: 1000 }, (_, i) => `127.0.0.1 host${i}.test`).join('\n'),
    },
  })
  const root = wrapper.find('section > div')
  const input = wrapper.find('[data-host-entry="l0"] input')
  ;(input.element as HTMLElement).focus()
  await input.trigger('compositionstart')
  ;(input.element as HTMLElement).blur()
  ;(root.element as HTMLElement).scrollTop = 6200
  await root.trigger('scroll')
  expect(wrapper.find('[data-host-entry="l0"] input').element).toBe(input.element)
  await input.trigger('compositionend')
  expect(wrapper.find('[data-host-entry="l0"]').exists()).toBe(false)
  const before = wrapper.find('[data-host-entry="l99"]')
  await before.findAll('button').at(-1)!.trigger('click')
  expect((root.element as HTMLElement).scrollTop).toBe(62 * 99)
  expect(wrapper.find('[data-host-entry="l100"]').exists()).toBe(true)
  expect(wrapper.findAll('[data-host-entry]').length).toBeLessThan(100)
})
