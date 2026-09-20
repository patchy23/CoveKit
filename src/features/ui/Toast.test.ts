import { createPinia, setActivePinia } from 'pinia'
import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import Toast from './Toast.vue'
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
