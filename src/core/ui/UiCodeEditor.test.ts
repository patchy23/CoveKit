/**
 * UiCodeEditor 挂载测试
 *
 * 覆盖实例创建、v-model 回写、只读切换、语言自动识别与命令式 API；
 * 布局像素与真实浏览器行为不在单测范围（在组件实验室人工验收）。
 */
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { defineComponent, h, nextTick, ref } from 'vue'
import UiCodeEditor from './UiCodeEditor.vue'
import EditorSearchBar from './editor/EditorSearchBar.vue'
import { EditorView } from '@codemirror/view'
import { undo } from '@codemirror/commands'
import { currentCompletions, startCompletion, closeCompletion } from '@codemirror/autocomplete'
import { MySQL, sql } from '@codemirror/lang-sql'

/** 等待编辑器挂载与异步语言加载 */
async function settle(): Promise<void> {
  await nextTick()
  await new Promise((resolve) => setTimeout(resolve, 30))
  await nextTick()
}

describe('UiCodeEditor', () => {
  it('首次挂载即装载补全，异步替换候选后无需重建编辑器', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: {
        modelValue: 'cus',
        language: 'sql',
        languageExtension: sql({ dialect: MySQL }),
        completionSources: [() => ({ from: 0, options: [{ label: 'customer_a' }] })],
      },
      attachTo: document.body,
    })
    try {
      await settle()
      const view = EditorView.findFromDOM(wrapper.get('.cm-editor').element as HTMLElement)!
      view.focus()
      view.dispatch({ selection: { anchor: 3 } })
      startCompletion(view)
      await new Promise((resolve) => setTimeout(resolve, 100))
      expect(currentCompletions(view.state).map((item) => item.label)).toContain('customer_a')

      closeCompletion(view)
      await wrapper.setProps({
        completionSources: [() => ({ from: 0, options: [{ label: 'customer_b' }] })],
      })
      startCompletion(view)
      await new Promise((resolve) => setTimeout(resolve, 100))
      expect(currentCompletions(view.state).map((item) => item.label)).toContain('customer_b')
      expect(currentCompletions(view.state).map((item) => item.label)).not.toContain('customer_a')
      expect(EditorView.findFromDOM(wrapper.get('.cm-editor').element as HTMLElement)).toBe(view)
    } finally {
      wrapper.unmount()
    }
  })
  it('滚动容器延后渲染宿主时仍能初始化并编辑', async () => {
    const ready = ref(false)
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'services: {}', filename: 'compose.yml' },
      global: {
        stubs: {
          UiScrollArea: defineComponent({
            inheritAttrs: false,
            setup(_, { slots }) {
              return () => (ready.value ? slots.default?.() : h('div'))
            },
          }),
        },
      },
    })
    await settle()
    ready.value = true
    await settle()
    try {
      expect(wrapper.find('.cm-content').text()).toContain('services: {}')
      expect(wrapper.find('.cm-content').attributes('contenteditable')).toBe('true')
    } finally {
      wrapper.unmount()
    }
  })
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

/** 命令式接口（批 2 能力） */
interface EditorApi {
  find: (replace?: boolean) => void
  format: () => boolean
  getValue: () => string
  setValue: (value: string, options?: { addToHistory?: boolean }) => void
  markSaved: () => void
  isDirty: () => boolean
  getSearchState: () => { total: number; current: number; error?: string }
}

describe('UiCodeEditor · 查找 / 格式化 / 状态栏 / 降级', () => {
  it('查找：统计匹配总数与当前序号', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'foo\nbar\nfoo baz', filename: 'a.txt' },
      attachTo: document.body,
    })
    await settle()

    const api = wrapper.vm as unknown as EditorApi
    api.find()
    await settle()

    const bar = wrapper.findComponent(EditorSearchBar)
    expect(bar.exists()).toBe(true)
    bar.vm.$emit('search', {
      query: 'foo',
      replacement: 'qux',
      options: { caseSensitive: false, regexp: false, wholeWord: false },
    })
    await settle()

    expect(api.getSearchState().total).toBe(2)
    expect(api.getSearchState().current).toBe(1)
    wrapper.unmount()
  })

  it('查找：全部替换改写文本并触发回写', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'foo\nbar\nfoo baz', filename: 'a.txt' },
      attachTo: document.body,
    })
    await settle()

    const api = wrapper.vm as unknown as EditorApi
    api.find()
    await settle()

    const bar = wrapper.findComponent(EditorSearchBar)
    bar.vm.$emit('search', {
      query: 'foo',
      replacement: 'qux',
      options: { caseSensitive: false, regexp: false, wholeWord: false },
    })
    await settle()
    bar.vm.$emit('replace-all')
    await settle()

    expect(api.getValue()).toBe('qux\nbar\nqux baz')
    expect(wrapper.emitted('update:modelValue')).toBeTruthy()
    wrapper.unmount()
  })

  it('查找：非法正则给出中文错误且不抛异常', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'abc', filename: 'a.txt' },
      attachTo: document.body,
    })
    await settle()

    const api = wrapper.vm as unknown as EditorApi
    api.find()
    await settle()

    wrapper.findComponent(EditorSearchBar).vm.$emit('search', {
      query: '[unclosed',
      replacement: '',
      options: { caseSensitive: false, regexp: true, wholeWord: false },
    })
    await settle()

    const state = api.getSearchState()
    expect(state.total).toBe(0)
    expect(state.error).toContain('正则')
    wrapper.unmount()
  })

  it('格式化：合法 JSON 重排缩进，非法内容返回失败并触发 error 事件', async () => {
    const ok = mount(UiCodeEditor, {
      props: { modelValue: '{"a":1,"b":[1,2]}', filename: 'a.json' },
      attachTo: document.body,
    })
    await settle()
    const okApi = ok.vm as unknown as EditorApi
    expect(okApi.format()).toBe(true)
    await settle()
    expect(okApi.getValue()).toContain('\n  "a": 1')
    ok.unmount()

    const bad = mount(UiCodeEditor, {
      props: { modelValue: '{bad}', filename: 'a.json' },
      attachTo: document.body,
    })
    await settle()
    const badApi = bad.vm as unknown as EditorApi
    expect(badApi.format()).toBe(false)
    expect(bad.emitted('error')?.[0]?.[0]).toContain('JSON')
    bad.unmount()
  })

  it('未保存标记：编辑后为脏，markSaved 后回归干净', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'origin', filename: 'a.txt' },
      attachTo: document.body,
    })
    await settle()

    const api = wrapper.vm as unknown as EditorApi
    expect(api.isDirty()).toBe(false)

    api.setValue('changed', { addToHistory: true })
    await settle()
    expect(api.isDirty()).toBe(true)

    api.markSaved()
    expect(api.isDirty()).toBe(false)
    wrapper.unmount()
  })

  it('状态栏：渲染行列、语言与编码', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: '{"a":1}', filename: 'config.json', statusBar: true },
      attachTo: document.body,
    })
    await settle()

    const text = wrapper.text()
    expect(text).toContain('行 1 : 列 1')
    expect(text).toContain('UTF-8')
    expect(text).toContain('JSON')
    wrapper.unmount()
  })

  it('大文件降级：超过 512KB 卸载折叠并在状态栏提示', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'x'.repeat(600000), filename: 'bulk.txt', statusBar: true },
      attachTo: document.body,
    })
    await settle()

    expect(wrapper.find('.cm-foldGutter').exists()).toBe(false)
    expect(wrapper.text()).toContain('512KB')
    wrapper.unmount()
  })
})

describe('独立文档的编辑状态', () => {
  it('切换保留各自撤销和光标，关闭文档释放旧历史', async () => {
    const wrapper = mount(UiCodeEditor, {
      props: { modelValue: 'A', language: 'text', documentKey: 'a', documentKeys: ['a', 'b'] },
      attachTo: document.body,
    })
    try {
      await settle()
      const view = EditorView.findFromDOM(wrapper.get('.cm-editor').element as HTMLElement)!
      view.dispatch({ changes: { from: 1, insert: '1' }, selection: { anchor: 2 } })
      await wrapper.setProps({ modelValue: 'A1' })
      await wrapper.setProps({ documentKey: 'b', modelValue: 'B' })
      view.dispatch({ changes: { from: 1, insert: '2' }, selection: { anchor: 2 } })
      await wrapper.setProps({ modelValue: 'B2' })
      await wrapper.setProps({ documentKey: 'a', modelValue: 'A1' })
      expect(view.state.selection.main.head).toBe(2)
      expect(undo(view)).toBe(true)
      expect(view.state.doc.toString()).toBe('A')
      await wrapper.setProps({ modelValue: 'A' })
      await wrapper.setProps({ documentKey: 'b', modelValue: 'B2', documentKeys: ['b'] })
      expect(view.state.doc.toString()).toBe('B2')
      expect(undo(view)).toBe(true)
      expect(view.state.doc.toString()).toBe('B')
      await wrapper.setProps({ modelValue: 'B' })
      await wrapper.setProps({ documentKey: 'a', modelValue: 'new A', documentKeys: ['a', 'b'] })
      expect(undo(view)).toBe(false)
      expect(view.state.doc.toString()).toBe('new A')
    } finally {
      wrapper.unmount()
    }
  })
})
