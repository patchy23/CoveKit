import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import QueryPanel from './QueryPanel.vue'
import type { ServerQueryResult } from './contracts'

const mock = vi.hoisted(() => ({
  query: vi.fn(),
  toast: vi.fn(),
  prepare: vi.fn(async () => 'read-id'),
  cancel: vi.fn(async () => {}),
}))
vi.mock('./ipc', () => ({
  ipc: { dnsQuery: mock.query, dnsReadPrepare: mock.prepare, dnsReadCancel: mock.cancel },
}))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ toast: mock.toast }) }))
vi.mock('@/core/ui', () => ({
  UiInput: {
    props: ['modelValue'],
    emits: ['update:modelValue'],
    template:
      '<input :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
  },
  UiButton: { props: ['loading'], template: '<button :disabled="loading"><slot /></button>' },
  UiIconButton: { template: '<button><slot /></button>' },
  UiSelect: { template: '<select />' },
  UiScrollArea: { template: '<div><slot /></div>' },
  UiTable: { template: '<table><slot /></table>' },
  UiTableCell: { template: '<td><slot /></td>' },
}))
afterEach(() => vi.resetAllMocks())

it('连续 Enter 只发一次查询，完成后新输入可立即查询', async () => {
  let resolve!: (value: ServerQueryResult[]) => void
  mock.query
    .mockReturnValueOnce(
      new Promise<ServerQueryResult[]>((yes) => {
        resolve = yes
      })
    )
    .mockResolvedValue([])
  const wrapper = mount(QueryPanel)
  try {
    const input = wrapper.get('input')
    await input.setValue('example.com')
    for (let i = 0; i < 30; i++) await input.trigger('keyup', { key: 'Enter' })
    expect(mock.query).toHaveBeenCalledTimes(1)
    resolve([{ server: 'system', ok: false, error: '测试结果', elapsedMs: 0, records: [] }])
    await flushPromises()
    expect(wrapper.text()).toContain('测试结果')
    await input.setValue('example.org')
    await input.trigger('keyup', { key: 'Enter' })
    expect(mock.query).toHaveBeenCalledTimes(2)
    expect(mock.query.mock.calls[1]?.[0]).toBe('example.org')
  } finally {
    wrapper.unmount()
  }
})

it('查询失败可见且可重试，关闭后的迟到失败不打扰其它工具', async () => {
  let reject!: (reason: Error) => void
  mock.query.mockRejectedValueOnce(new Error('首次失败')).mockReturnValueOnce(
    new Promise((_, no) => {
      reject = no
    })
  )
  const wrapper = mount(QueryPanel)
  await wrapper.get('input').setValue('example.com')
  await wrapper.get('input').trigger('keyup', { key: 'Enter' })
  await flushPromises()
  expect(mock.toast).toHaveBeenCalledWith('查询失败：首次失败')
  await wrapper.get('input').trigger('keyup', { key: 'Enter' })
  await flushPromises()
  wrapper.unmount()
  expect(mock.cancel).toHaveBeenCalledWith('read-id')
  reject(new Error('迟到失败'))
  await flushPromises()
  expect(mock.toast).toHaveBeenCalledTimes(1)
})
