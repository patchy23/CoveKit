import { mount } from '@vue/test-utils'
import { defineComponent } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import JsonPanel from './JsonPanel.vue'
import { formatJson } from '@/core/format/json'

vi.mock('@/core/feedback/useCopy', () => ({ useCopy: () => ({ copyText: vi.fn() }) }))
vi.mock('@/core/ui', () => ({
  UiAlert: { template: '<div><slot /></div>' },
  UiToolbar: { template: '<div><slot /></div>' },
  UiButton: { template: '<button><slot /></button>' },
  UiRadioGroup: { template: '<div />' },
  UiCodeEditor: defineComponent({
    props: { modelValue: { type: String, default: '' } },
    emits: ['update:modelValue'],
    template:
      '<textarea :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
  }),
}))
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

it('保留自动转换时机，大输入计算可清空且旧结果不会回填', async () => {
  vi.useFakeTimers()
  const workers: {
    onmessage?: (event: { data: unknown }) => void
    postMessage: ReturnType<typeof vi.fn>
    terminate: ReturnType<typeof vi.fn>
  }[] = []
  vi.stubGlobal(
    'Worker',
    class {
      postMessage = vi.fn()
      terminate = vi.fn()
      constructor() {
        workers.push(this)
      }
    }
  )
  const wrapper = mount(JsonPanel)
  try {
    const [input, output] = wrapper.findAll('textarea')
    const text = JSON.stringify({ text: 'x'.repeat(70000) })
    await input.setValue(text)
    await vi.advanceTimersByTimeAsync(299)
    expect(workers).toHaveLength(0)
    await vi.advanceTimersByTimeAsync(1)
    expect(workers).toHaveLength(1)
    expect(wrapper.text()).toContain('正在转换')
    const request = workers[0].postMessage.mock.lastCall![0]
    await wrapper.findAll('button')[0].trigger('click')
    expect(workers[0].terminate).toHaveBeenCalledOnce()
    workers[0].onmessage?.({ data: { id: request.id, result: formatJson(text) } })
    await vi.advanceTimersByTimeAsync(301)
    expect((output.element as HTMLTextAreaElement).value).toBe('')
    await input.setValue('{"ok":true}')
    await vi.advanceTimersByTimeAsync(300)
    expect((output.element as HTMLTextAreaElement).value).toBe('{\n  "ok": true\n}')
  } finally {
    wrapper.unmount()
  }
})
