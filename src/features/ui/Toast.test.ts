import { createPinia, setActivePinia } from 'pinia'
import { DOMWrapper, enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import Toast from './Toast.vue'
import UiModal from '@/core/ui/UiModal.vue'
import { useUiStore } from '@/stores/ui'
import { writeClipboardText } from '@/core/platform/clipboard'

vi.mock('@/core/platform/clipboard', () => ({ writeClipboardText: vi.fn() }))
enableAutoUnmount(afterEach)
beforeEach(() => {
  setActivePinia(createPinia())
  vi.useFakeTimers()
  vi.mocked(writeClipboardText).mockResolvedValue({ ok: true })
})
afterEach(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
})
function setup() {
  const ui = useUiStore()
  const wrapper = mount(Toast, {
    global: {
      stubs: {
        teleport: true,
        UiModal: {
          props: ['open'],
          template: '<section v-if="open"><slot /><slot name="footer" /></section>',
        },
      },
    },
  })
  return { ui, wrapper }
}
it('悬停与焦点共同保留提示，期间的新消息也不自动消失，离开后恢复计时', async () => {
  const { ui, wrapper } = setup()
  ui.toast('失败信息')
  await nextTick()
  const toast = wrapper.get('.fixed')
  await toast.trigger('mouseenter')
  await toast.trigger('focusin')
  await toast.trigger('mouseleave')
  ui.toast('新的失败信息')
  await vi.advanceTimersByTimeAsync(5000)
  expect(ui.toastVisible).toBe(true)
  await toast.trigger('focusout', { relatedTarget: null })
  await vi.advanceTimersByTimeAsync(1601)
  expect(ui.toastVisible).toBe(false)
})
it('详情保持所打开消息的快照，复制不被后来的提示覆盖', async () => {
  const { ui, wrapper } = setup()
  ui.toast('原始错误\n完整路径 /tmp/example')
  await nextTick()
  await wrapper
    .findAll('button')
    .find((button) => button.text() === '详情')!
    .trigger('click')
  expect(ui.toastVisible).toBe(false)
  ui.toast('后来的消息')
  await vi.advanceTimersByTimeAsync(5000)
  expect(wrapper.get('section p').text()).toContain('原始错误')
  await wrapper
    .findAll('button')
    .find((button) => button.text() === '复制内容')!
    .trigger('click')
  await flushPromises()
  expect(writeClipboardText).toHaveBeenCalledWith('原始错误\n完整路径 /tmp/example')
  expect(wrapper.get('section').text()).toContain('已复制')
  vi.mocked(writeClipboardText).mockResolvedValue({ ok: false, reason: 'failed' })
  await wrapper
    .findAll('button')
    .find((button) => button.text() === '复制内容')!
    .trigger('click')
  await flushPromises()
  expect(wrapper.get('section').text()).toContain('复制失败，请选择正文手动复制')
})

it('后打开的消息详情置于业务弹窗之上，关闭详情后业务弹窗仍可操作', async () => {
  // 全局 Toast 先于随后进入的工具弹窗挂载，保留真实 Portal 和 Reka 焦点层。
  const toast = mount(Toast, { attachTo: document.body })
  const modal = mount(UiModal, {
    attachTo: document.body,
    props: { open: true, title: '业务弹窗', description: '输入内容' },
    slots: { default: '<input aria-label="原表单" value="保留内容" />' },
  })
  await flushPromises()
  const original = document.querySelector<HTMLElement>('[role="dialog"]')!
  const input = original.querySelector('input')!
  const ui = useUiStore()
  for (const closeByEscape of [false, true]) {
    ui.toast('错误的完整详情')
    await nextTick()
    const button = Array.from(document.querySelectorAll('button')).find(
      (node) => node.textContent === '详情'
    )!
    await new DOMWrapper(button).trigger('click')
    await flushPromises()
    await vi.advanceTimersByTimeAsync(20)
    await flushPromises()
    expect(ui.toastVisible).toBe(false)
    const details = Array.from(document.querySelectorAll<HTMLElement>('[role="dialog"]')).find(
      (node) => node !== original
    )!
    expect(original.compareDocumentPosition(details) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0)
    expect(details.style.pointerEvents).not.toBe('none')
    const copy = Array.from(details.querySelectorAll('button')).find(
      (node) => node.textContent === '复制内容'
    )!
    copy.focus()
    expect(document.activeElement).toBe(copy)
    await new DOMWrapper(copy).trigger('click')
    await flushPromises()
    expect(writeClipboardText).toHaveBeenCalledWith('错误的完整详情')
    if (closeByEscape) {
      await new DOMWrapper(copy).trigger('keydown', { key: 'Escape' })
    } else {
      await new DOMWrapper(
        details.querySelector<HTMLButtonElement>('button[aria-label="关闭"]')!
      ).trigger('click')
    }
    await flushPromises()
    await vi.advanceTimersByTimeAsync(1)
    expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(1)
    expect(modal.emitted('close')).toBeUndefined()
    input.focus()
    expect(document.activeElement).toBe(input)
    expect(input.value).toBe('保留内容')
  }
  toast.unmount()
  modal.unmount()
})
