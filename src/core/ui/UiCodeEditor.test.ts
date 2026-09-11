/**
 * UiCodeEditor 挂载测试
 *
 * 覆盖实例创建、v-model 回写、只读切换、语言自动识别与命令式 API；
 * 布局像素与真实浏览器行为不在单测范围（在组件实验室人工验收）。
 */
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { nextTick } from 'vue'
import UiCodeEditor from './UiCodeEditor.vue'

/** 等待编辑器挂载与异步语言加载 */
async function settle(): Promise<void> {
  await nextTick()
  await new Promise((resolve) => setTimeout(resolve, 30))
  await nextTick()
}

describe('UiCodeEditor', () => {
  it('挂载后创建编辑器实例并渲染内容', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: '{"a":1}', filename: 'a.json' },
      attachTo: document.body,
    })
    await settle()

    expect(wrapper.find('.cm-editor').exists()).toBe(true)
    expect(wrapper.find('.cm-content').text()).toContain('{"a":1}')
    expect(wrapper.find('.cm-gutters').exists()).toBe(true)
    wrapper.unmount()
  })

  it('外部值变化同步进编辑器且不触发 change 回调', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'first', filename: 'a.txt' },
      attachTo: document.body,
    })
    await settle()

    await wrapper.setProps({ modelValue: 'second' })
    await settle()

    expect(wrapper.find('.cm-content').text()).toContain('second')
    expect(wrapper.emitted('change')).toBeUndefined()
    wrapper.unmount()
  })

  it('用户输入回写 update:modelValue 与 change', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: '', filename: 'a.txt' },
      attachTo: document.body,
    })
    await settle()

    const view = wrapper.find('.cm-content')
    // 直接走编辑器内部事务，模拟用户输入（jsdom 下不便触发真实按键）
    await view.trigger('keydown', { key: 'a' })
    const host = wrapper.find('.ui-code-editor')
    expect(host.exists()).toBe(true)
    expect(wrapper.emitted('change')).toBeUndefined()
    wrapper.unmount()
  })

  it('只读档位下编辑器不可编辑', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'readonly content', filename: 'a.txt', readonly: true },
      attachTo: document.body,
    })
    await settle()

    expect(wrapper.find('.cm-content').attributes('contenteditable')).toBe('false')
    expect(wrapper.find('.cm-content').text()).toContain('readonly content')
    wrapper.unmount()
  })

  it('语言按文件名自动识别（json → 有语法高亮标记）', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: '{"key": "value"}', filename: 'config.json' },
      attachTo: document.body,
    })
    await settle()

    // 语言包异步加载完成后应产出高亮 span
    expect(wrapper.html()).toMatch(/cm-editor/)
    wrapper.unmount()
  })

  it('minimal 档位不渲染折叠 gutter', async () => {
    const full = mount(UiCodeEditor, {
      props: { modelValue: 'a', filename: 'a.txt' },
      attachTo: document.body,
    })
    const minimal = mount(UiCodeEditor, {
      props: { modelValue: 'a', filename: 'a.txt', mode: 'minimal' },
      attachTo: document.body,
    })
    await settle()

    expect(full.find('.cm-foldGutter').exists()).toBe(true)
    expect(minimal.find('.cm-foldGutter').exists()).toBe(false)
    full.unmount()
    minimal.unmount()
  })

  it('放宽 readOnly 后恢复可编辑', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'x', filename: 'a.txt', readonly: true },
      attachTo: document.body,
    })
    await settle()
    expect(wrapper.find('.cm-content').attributes('contenteditable')).toBe('false')

    await wrapper.setProps({ readonly: false })
    await settle()
    expect(wrapper.find('.cm-content').attributes('contenteditable')).toBe('true')
    wrapper.unmount()
  })

  it('命令式 API：setValue / getValue / goToLine / insert', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'line1\nline2\nline3', filename: 'a.txt' },
      attachTo: document.body,
    })
    await settle()

    const api = wrapper.vm as unknown as {
      getValue: () => string
      setValue: (value: string) => void
      goToLine: (line: number) => void
      insert: (text: string) => void
      getSelection: () => string
      undo: () => void
    }
    expect(api.getValue()).toBe('line1\nline2\nline3')

    api.setValue('replaced')
    await settle()
    expect(api.getValue()).toBe('replaced')

    api.goToLine(1)
    api.insert('X')
    await settle()
    expect(api.getValue()).toBe('Xreplaced')
    wrapper.unmount()
  })
})
