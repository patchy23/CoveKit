import { afterEach, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import XmlPanel from './XmlPanel.vue'

vi.mock('@/core/feedback/useCopy', () => ({ useCopy: () => ({ copyText: vi.fn() }) }))
vi.mock('@/core/ui', () => {
  const Slot = { template: '<div><slot /></div>' }
  return {
    UiAlert: Slot,
    UiButton: Slot,
    UiToolbar: Slot,
    UiRadioGroup: {
      props: { modelValue: { type: String, default: '' } },
      emits: ['update:modelValue'],
      template:
        "<button @click=\"$emit('update:modelValue', modelValue === 'format' ? 'minify' : 'format')\">切换模式</button>",
    },
    UiCodeEditor: {
      props: { modelValue: { type: String, default: '' } },
      emits: ['update:modelValue'],
      template:
        '<textarea :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
    },
  }
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

it('输入变化立即取消，模式切换立即重算，关闭释放线程且旧结果不覆盖输出', async () => {
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
  const wrapper = mount(XmlPanel)
  try {
    const [input, output] = wrapper.findAll('textarea')
    const large = `<a>${'x'.repeat(70000)}</a>`
    await input.setValue(large)
    await vi.advanceTimersByTimeAsync(300)
    expect(workers[0].postMessage.mock.lastCall![0].mode).toBe('format')
    expect(wrapper.text()).toContain('正在转换')
    await wrapper.get('button').trigger('click')
    expect(workers[0].terminate).toHaveBeenCalledOnce()
    expect(workers[1].postMessage.mock.lastCall![0].mode).toBe('minify')
    await input.setValue('<a>  <b/> </a>')
    expect(workers[1].terminate).toHaveBeenCalledOnce()
    for (const worker of workers)
      worker.onmessage?.({
        data: {
          id: worker.postMessage.mock.lastCall![0].id,
          result: { ok: true, output: 'STALE' },
        },
      })
    await vi.advanceTimersByTimeAsync(300)
    expect((output.element as HTMLTextAreaElement).value).toBe('<a><b/></a>')
    expect(wrapper.text()).not.toContain('转换失败')
    await input.setValue(large)
    await vi.advanceTimersByTimeAsync(300)
  } finally {
    wrapper.unmount()
  }
  expect(workers[2].terminate).toHaveBeenCalledOnce()
})
