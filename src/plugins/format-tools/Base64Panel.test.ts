import { describe, it, expect, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, nextTick } from 'vue'
import Base64Panel from './Base64Panel.vue'

vi.mock('@/core/feedback/useCopy', () => ({ useCopy: () => ({ copyText: vi.fn() }) }))
// 仅替换重型编辑器与展示壳，测试真实面板的输入调度和编码输出。
vi.mock('@/core/ui', () => {
  const Slot = { template: '<div><slot /></div>' }
  return {
    UiAlert: Slot,
    UiButton: Slot,
    UiToolbar: Slot,
    UiRadioGroup: Slot,
    UiCodeEditor: defineComponent({
      props: { modelValue: { type: String, default: '' } },
      emits: ['update:modelValue'],
      template:
        '<textarea :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
    }),
  }
})

describe('Base64 面板自动编码', () => {
  it.each([
    [' ', 'IA=='],
    ['\n', 'Cg=='],
    ['\t', 'CQ=='],
  ])('保留纯空白输入 %j 的字节', async (input, expected) => {
    vi.useFakeTimers()
    const wrapper = mount(Base64Panel)
    try {
      await wrapper.findAll('textarea')[0].setValue(input)
      await vi.advanceTimersByTimeAsync(301)
      await nextTick()
      expect((wrapper.findAll('textarea')[1].element as HTMLTextAreaElement).value).toBe(expected)
      await wrapper.findAll('textarea')[0].setValue('')
      await vi.advanceTimersByTimeAsync(301)
      await nextTick()
      expect((wrapper.findAll('textarea')[1].element as HTMLTextAreaElement).value).toBe('')
    } finally {
      wrapper.unmount()
      vi.useRealTimers()
    }
  })
})
