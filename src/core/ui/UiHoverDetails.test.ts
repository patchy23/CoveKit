import { afterEach, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import UiHoverDetails from './UiHoverDetails.vue'

afterEach(() => {
  vi.useRealTimers()
  document.body.innerHTML = ''
})

it('悬停延迟展开且不抢焦点，移入详情保持，移出后关闭', async () => {
  vi.useFakeTimers()
  const input = document.createElement('input')
  document.body.append(input)
  input.focus()
  const wrapper = mount(UiHoverDetails, {
    attachTo: document.body,
    props: { label: '资源详情' },
    slots: { trigger: 'CPU 2%', default: '内存 100 MiB' },
  })
  await wrapper.find('span.block').trigger('pointerenter', { pointerType: 'mouse', buttons: 0 })
  await vi.advanceTimersByTimeAsync(399)
  expect(document.querySelector('[role="dialog"]')).toBeNull()
  await vi.advanceTimersByTimeAsync(1)
  await flushPromises()
  expect(document.querySelector('[role="dialog"]')).not.toBeNull()
  expect(document.activeElement).toBe(input)
  await wrapper.find('span.block').trigger('pointerleave')
  document.querySelector('[role="dialog"]')!.dispatchEvent(new Event('pointerenter'))
  await vi.advanceTimersByTimeAsync(300)
  expect(document.querySelector('[role="dialog"]')).not.toBeNull()
  document.querySelector('[role="dialog"]')!.dispatchEvent(new Event('pointerleave'))
  await vi.advanceTimersByTimeAsync(200)
  expect(document.querySelector('[role="dialog"]')).toBeNull()
  wrapper.unmount()
})

it('点击保持展开，第二次点击关闭，卸载取消未触发悬停', async () => {
  vi.useFakeTimers()
  const wrapper = mount(UiHoverDetails, {
    attachTo: document.body,
    props: { label: '资源详情' },
    slots: { trigger: 'CPU', default: '详情' },
  })
  await wrapper.find('button').trigger('click')
  await wrapper.find('span.block').trigger('pointerleave')
  await vi.advanceTimersByTimeAsync(500)
  expect(document.querySelector('[role="dialog"]')).not.toBeNull()
  await wrapper.find('button').trigger('pointerdown', { pointerType: 'mouse', button: 0 })
  await wrapper.find('button').trigger('click')
  await flushPromises()
  expect(document.querySelector('[role="dialog"]')).toBeNull()
  await wrapper.find('span.block').trigger('pointerenter', { pointerType: 'mouse', buttons: 0 })
  wrapper.unmount()
  await vi.advanceTimersByTimeAsync(500)
  expect(document.querySelector('[role="dialog"]')).toBeNull()
})
