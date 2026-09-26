import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import { newDraft } from './requestDraft'
import { useRequestSession } from './useRequestSession'
import StreamMessages from './StreamMessages.vue'

vi.mock('@/core/platform/clipboard', () => ({
  writeClipboardText: vi.fn(async () => ({ ok: true })),
}))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ toast: vi.fn() }) }))
vi.mock('@/core/ui', () => ({
  UiButton: { template: '<button><slot /></button>' },
  UiCheckbox: { template: '<span />' },
  UiCodeEditor: { template: '<textarea />' },
  UiInput: {
    props: ['modelValue'],
    emits: ['update:modelValue'],
    template:
      '<input :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
  },
  UiScrollArea: { template: '<div><slot /></div>' },
  UiSelect: { template: '<select />' },
}))
import { writeClipboardText } from '@/core/platform/clipboard'

describe('流消息完整历史展示', () => {
  it('折叠历史仍可全文搜索和复制，追加快照保留已有展开节点，清空释放所有行', async () => {
    const session = useRequestSession(() => newDraft('ws'), vi.fn())
    const first = {
      seq: 1,
      time: 1,
      direction: 'received',
      kind: 'text',
      eventId: '',
      content: '  Hidden Needle  ',
    }
    session.state.entries = [first, { ...first, seq: 2, content: 'other' }]
    const wrapper = mount(StreamMessages, { props: { session, kind: 'ws', active: false } })
    try {
      expect(wrapper.findAll('details')).toHaveLength(2)
      expect(wrapper.findAll('pre')).toHaveLength(0)
      await wrapper.get('input').setValue('needle')
      expect(wrapper.findAll('details')).toHaveLength(1)
      await wrapper
        .findAll('button')
        .find((button) => button.text() === '复制')!
        .trigger('click')
      expect(writeClipboardText).toHaveBeenCalledWith(first.content)
      await wrapper.get('input').setValue('')
      const row = wrapper.find('details')
      ;(row.element as HTMLDetailsElement).open = true
      await row.trigger('toggle')
      const pre = wrapper.get('pre').element
      session.state.entries = [
        ...session.state.entries.map((entry) => ({ ...entry })),
        { ...first, seq: 3 },
      ]
      await nextTick()
      expect(wrapper.get('pre').element).toBe(pre)
      await wrapper
        .findAll('button')
        .find((button) => button.text() === '填入发送框')!
        .trigger('click')
      expect(session.state.message).toBe(first.content)
      await wrapper
        .findAll('button')
        .find((button) => button.text() === '清空')!
        .trigger('click')
      expect(wrapper.findAll('details')).toHaveLength(0)
    } finally {
      wrapper.unmount()
      await session.dispose()
    }
  })
})
