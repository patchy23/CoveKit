import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { reactive } from 'vue'
import RuntimeLogPanel from './RuntimeLogPanel.vue'
import type { FrpLogLine } from './frpStatus'

vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('@/core/ui', () => ({
  UiButton: { template: '<button><slot /></button>' },
  UiIconButton: { template: '<button><slot /></button>' },
  UiIcon: { template: '<i />' },
  UiToolbar: { template: '<header><slot /></header>' },
  UiScrollArea: { template: '<section><slot /></section>' },
  UiInput: {
    props: ['modelValue'],
    emits: ['update:modelValue'],
    template:
      '<input :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
  },
}))

enableAutoUnmount(afterEach)
afterEach(() => vi.restoreAllMocks())

function line(text: string): FrpLogLine {
  return { ts: 1000, line: text, stream: 'stdout', level: 'info' }
}

it('同毫秒日志裁剪和过滤后保留幸存行节点，追加只格式化新行', async () => {
  const format = vi.fn((ts: number) => String(ts))
  const formatter = new Intl.DateTimeFormat()
  Object.defineProperty(formatter, 'format', { value: format })
  vi.spyOn(Intl, 'DateTimeFormat').mockImplementation(function () {
    return formatter
  })
  const first = line('first')
  const second = line('needle')
  const wrapper = mount(RuntimeLogPanel, { props: { lines: [first, second], running: true } })
  const rows = () => wrapper.findAll('div.select-text')
  const survivor = rows()[1]!.element
  expect(format).toHaveBeenCalledTimes(2)
  await wrapper.setProps({ lines: [second, line('third')] })
  expect(rows()[0]!.element).toBe(survivor)
  expect(format).toHaveBeenCalledTimes(3)
  await wrapper.get('input').setValue('NEEDLE')
  expect(rows()).toHaveLength(1)
  expect(rows()[0]!.element).toBe(survivor)
  await wrapper.get('input').setValue('')
  expect(rows()).toHaveLength(2)
  expect(format).toHaveBeenCalledTimes(3)
  await wrapper.setProps({ lines: [] })
  expect(rows()).toHaveLength(0)
})

it('缓冲长度不变时继续跟随新日志，阅读历史时不强制滚动', async () => {
  const wrapper = mount(RuntimeLogPanel, { props: { lines: [line('first')], running: true } })
  const scroller = wrapper.get('section > div')
  const element = scroller.element as HTMLElement
  Object.defineProperties(element, {
    scrollHeight: { configurable: true, value: 1000 },
    clientHeight: { configurable: true, value: 100 },
  })
  await wrapper.setProps({ lines: [line('second')] })
  await wrapper.vm.$nextTick()
  expect(element.scrollTop).toBe(1000)
  element.scrollTop = 20
  await scroller.trigger('scroll')
  await wrapper.setProps({ lines: [line('third')] })
  await wrapper.vm.$nextTick()
  expect(element.scrollTop).toBe(20)
})

it('持续追加和裁剪只搜索新正文，切换关键词及原地修改仍重新匹配', async () => {
  const first = reactive(line('ERROR first'))
  const second = line('INFO second')
  const third = line('ERROR third')
  const wrapper = mount(RuntimeLogPanel, { props: { lines: [first, second], running: true } })
  const rows = () => wrapper.findAll('div.select-text').map((row) => row.text())
  await wrapper.get('input').setValue('error')
  const lower = vi.spyOn(String.prototype, 'toLowerCase')
  await wrapper.setProps({ lines: [first, second, third] })
  expect(rows()).toHaveLength(2)
  expect(lower.mock.contexts).not.toContain(first.line)
  expect(lower.mock.contexts).not.toContain(second.line)
  expect(lower.mock.contexts).toContain(third.line)
  lower.mockClear()
  await wrapper.setProps({ lines: [first, third] })
  expect(rows()).toHaveLength(2)
  expect(lower.mock.contexts).not.toContain(third.line)
  first.line = 'INFO changed'
  await wrapper.vm.$nextTick()
  expect(rows()).toHaveLength(1)
  expect(rows()[0]).toContain('ERROR third')
  await wrapper.get('input').setValue('INFO')
  expect(rows()).toHaveLength(1)
  expect(rows()[0]).toContain('INFO changed')
  await wrapper.get('input').setValue('')
  expect(rows()).toHaveLength(2)
  await wrapper.setProps({ lines: [] })
  await wrapper.get('input').setValue('error')
  await wrapper.setProps({ lines: [third] })
  expect(rows()[0]).toContain('ERROR third')
})
