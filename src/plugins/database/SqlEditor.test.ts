import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { describe, expect, it, vi } from 'vitest'
import { EditorView } from '@codemirror/view'
import { currentCompletions, startCompletion, closeCompletion } from '@codemirror/autocomplete'
import SqlEditor from './SqlEditor.vue'

describe('SQL 编辑器交互', () => {
  it('首次输入自动弹出 SQL 关键字，异步表名更新后可补全', async () => {
    const wrapper = mount(SqlEditor, {
      props: { modelValue: '', dialect: 'mysql', tables: [] },
      attachTo: document.body,
    })
    try {
      await nextTick()
      const view = EditorView.findFromDOM(wrapper.get('.cm-editor').element as HTMLElement)!
      view.focus()
      view.dispatch({
        changes: { from: 0, insert: 'sel' },
        selection: { anchor: 3 },
        userEvent: 'input.type',
      })
      await vi.waitFor(() => {
        expect(
          currentCompletions(view.state).some((item) => item.label.toUpperCase() === 'SELECT')
        ).toBe(true)
      })

      closeCompletion(view)
      await wrapper.setProps({ tables: [{ name: 'customers', columns: [] }] })
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: 'SELECT * FROM cus' },
        selection: { anchor: 17 },
      })
      startCompletion(view)
      await vi.waitFor(() => {
        expect(currentCompletions(view.state).map((item) => item.label)).toContain('customers')
      })
    } finally {
      wrapper.unmount()
    }
  })

  it('执行边栏紧邻行号，点击仍执行对应完整语句', async () => {
    const onRunStatement = vi.fn()
    const wrapper = mount(SqlEditor, {
      props: { modelValue: 'SELECT 1;\nSELECT 2;', dialect: 'mysql', onRunStatement },
      attachTo: document.body,
    })
    try {
      await nextTick()
      const gutters = wrapper.findAll('.cm-gutter')
      expect(gutters[0].classes()).toContain('cm-run-statement-gutter')
      expect(gutters[1].classes()).toContain('cm-lineNumbers')
      expect(wrapper.findAll('.cm-run-statement-btn')).toHaveLength(2)
      // DOM 单测没有真实行坐标；首行点击验证入口，多语句定位由范围解析测试覆盖。
      await wrapper.findAll('.cm-run-statement-btn')[0].trigger('mousedown', { button: 0 })
      expect(onRunStatement).toHaveBeenCalledWith('SELECT 1')
    } finally {
      wrapper.unmount()
    }
  })

  it('默认执行优先使用选区，失焦后仍保留选中 SQL', async () => {
    const onRunStatement = vi.fn()
    const text = 'SELECT 1;\nSELECT 2;'
    const wrapper = mount(SqlEditor, {
      props: { modelValue: text, dialect: 'mysql', onRunStatement },
      attachTo: document.body,
    })
    try {
      await nextTick()
      const view = EditorView.findFromDOM(wrapper.get('.cm-editor').element as HTMLElement)!
      // 没有选区时仍按光标所在语句执行。
      expect(wrapper.vm.getExecutableSql()).toBe('SELECT 1')

      const selectedSql = 'SELECT 2'
      const selectedFrom = text.indexOf(selectedSql)
      view.dispatch({
        selection: { anchor: selectedFrom, head: selectedFrom + selectedSql.length },
      })
      await nextTick()

      // Ctrl+Enter 是默认执行快捷键；执行回调读取选中部分且保留选区。
      view.focus()
      view.contentDOM.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Enter',
          ctrlKey: true,
          bubbles: true,
          cancelable: true,
        })
      )
      expect(onRunStatement).toHaveBeenLastCalledWith(selectedSql)
      expect(view.state.selection.main.from).toBe(selectedFrom)
      expect(view.state.selection.main.to).toBe(selectedFrom + selectedSql.length)

      // 明确的 Ctrl+Shift+Enter 仍执行整份文档。
      view.contentDOM.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Enter',
          ctrlKey: true,
          shiftKey: true,
          bubbles: true,
          cancelable: true,
        })
      )
      expect(onRunStatement).toHaveBeenLastCalledWith(text)

      view.contentDOM.blur()
      await nextTick()
      expect(wrapper.vm.getExecutableSql()).toBe(selectedSql)
      expect(view.state.selection.main.from).toBe(selectedFrom)
      expect(view.state.selection.main.to).toBe(selectedFrom + selectedSql.length)
    } finally {
      wrapper.unmount()
    }
  })

  it('空白选区不会回退到执行整篇文档', async () => {
    const text = 'SELECT 1;\nSELECT 2;'
    const wrapper = mount(SqlEditor, {
      props: { modelValue: text, dialect: 'mysql' },
      attachTo: document.body,
    })
    try {
      await nextTick()
      const view = EditorView.findFromDOM(wrapper.get('.cm-editor').element as HTMLElement)!
      view.dispatch({ selection: { anchor: 9, head: 10 } })
      expect(wrapper.vm.getExecutableSql()).toBe('\n')
    } finally {
      wrapper.unmount()
    }
  })
})
