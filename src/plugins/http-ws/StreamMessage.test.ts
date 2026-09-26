import { mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import StreamMessage from './StreamMessage.vue'
import type { StreamEntry } from './useRequestSession'

vi.mock('@/core/ui', () => ({ UiButton: { template: '<button><slot /></button>' } }))
afterEach(() => vi.restoreAllMocks())
const entry: StreamEntry = {
  seq: 1,
  time: 1000,
  direction: 'received',
  kind: 'text',
  eventId: '',
  content: '{"value":1}',
}

describe('流消息按展开格式化', () => {
  it('折叠不解析，重复快照与重展复用结果，正文变化才重新解析', async () => {
    const parse = vi.spyOn(JSON, 'parse')
    const wrapper = mount(StreamMessage, { props: { entry, kind: 'ws' } })
    const count = () => parse.mock.calls.filter(([text]) => text === entry.content).length
    const toggle = async (open: boolean) => {
      ;(wrapper.element as HTMLDetailsElement).open = open
      await wrapper.trigger('toggle')
    }
    try {
      expect(wrapper.find('pre').exists()).toBe(false)
      expect(count()).toBe(0)
      await toggle(true)
      expect(wrapper.get('pre').text()).toBe('{\n  "value": 1\n}')
      expect(count()).toBe(1)
      await wrapper.setProps({ entry: { ...entry } })
      expect(count()).toBe(1)
      await toggle(false)
      expect(wrapper.find('pre').exists()).toBe(false)
      await toggle(true)
      expect(count()).toBe(1)
      await wrapper.setProps({ entry: { ...entry, content: '{"value":2}' } })
      expect(wrapper.get('pre').text()).toBe('{\n  "value": 2\n}')
      expect(count()).toBe(1)
      expect(parse.mock.calls.filter(([text]) => text === '{"value":2}')).toHaveLength(1)
    } finally {
      wrapper.unmount()
    }
  })

  it('复制和回填保留原始空白，非 JSON 正文完整显示', async () => {
    const content = '  raw\ntext  '
    const wrapper = mount(StreamMessage, { props: { entry: { ...entry, content }, kind: 'ws' } })
    try {
      ;(wrapper.element as HTMLDetailsElement).open = true
      await wrapper.trigger('toggle')
      expect(wrapper.get('pre').element.textContent).toBe(content)
      const buttons = wrapper.findAll('button')
      await buttons[0]!.trigger('click')
      await buttons[1]!.trigger('click')
      expect(wrapper.emitted('copy')).toEqual([[content]])
      expect(wrapper.emitted('fill')).toEqual([[content]])
      await wrapper.setProps({ kind: 'sse' })
      expect(wrapper.findAll('button')).toHaveLength(1)
    } finally {
      wrapper.unmount()
    }
  })
})
