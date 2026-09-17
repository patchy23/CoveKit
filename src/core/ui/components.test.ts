import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import UiButton from './UiButton.vue'
import UiInput from './UiInput.vue'
import UiTabs from './UiTabs.vue'
import UiCheckbox from './UiCheckbox.vue'
import UiPagination from './UiPagination.vue'
import UiSelect from './UiSelect.vue'
import UiSwitch from './UiSwitch.vue'
import UiTableCell from './UiTableCell.vue'
import UiSearchInput from './UiSearchInput.vue'
import UiModal from './UiModal.vue'
import UiCombobox from './UiCombobox.vue'
import UiTree from './UiTree.vue'
import UiDataGrid from './UiDataGrid.vue'
import UiSplitPane from './UiSplitPane.vue'

const pluginVueSources = import.meta.glob('../../plugins/**/*.vue', {
  eager: true,
  query: '?raw',
  import: 'default',
}) as Record<string, string>

describe('公共 UI 组件', () => {
  it('按钮在加载中自动禁用并显示加载状态', () => {
    const wrapper = mount(UiButton, { props: { loading: true }, slots: { default: '保存' } })
    expect(wrapper.get('button').attributes('disabled')).toBeDefined()
    expect(wrapper.find('.ui-spinner').exists()).toBe(true)
  })

  it('输入框支持 trim 和 number 修饰符', async () => {
    const wrapper = mount(UiInput, { props: { modelModifiers: { trim: true, number: true } } })
    await wrapper.get('input').setValue(' 42 ')
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([42])
  })

  it('页签点击后更新受控值', async () => {
    const wrapper = mount(UiTabs, {
      props: {
        modelValue: 'first',
        items: [
          { value: 'first', label: '第一项' },
          { value: 'second', label: '第二项' },
        ],
      },
    })
    const tabs = wrapper.findAll('[role="tab"]')
    await tabs[1].trigger('mousedown', { button: 0, ctrlKey: false })
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['second'])
  })

  it('可关闭页签通过统一事件请求关闭', async () => {
    const wrapper = mount(UiTabs, {
      props: {
        modelValue: 'terminal-1',
        items: [{ value: 'terminal-1', label: '生产服务器', closable: true }],
      },
    })
    await wrapper.get('[aria-label="关闭生产服务器"]').trigger('click')
    expect(wrapper.emitted('close')?.[0]).toEqual(['terminal-1'])
  })

  it('基础控件输出统一尺寸类', () => {
    expect(mount(UiButton, { props: { size: 'xs' } }).classes()).toContain('ui-control-xs')
    expect(
      mount(UiInput, { props: { size: 'lg' } })
        .get('input')
        .classes()
    ).toContain('ui-control-lg')
    expect(
      mount(UiSelect, {
        props: { modelValue: 'a', options: [{ value: 'a' }], size: 'sm' },
      })
        .get('button')
        .classes()
    ).toContain('ui-control-sm')
  })

  it('下拉框收起值使用次级文字色', () => {
    const select = mount(UiSelect, {
      props: { modelValue: 'cpu', options: [{ value: 'cpu', label: '按 CPU' }] },
    })
    expect(select.get('[role="combobox"]').classes()).toContain('text-secondary')
    expect(select.get('[role="combobox"]').classes()).not.toContain('text-primary')
  })

  it('复选框和开关保持受控更新', async () => {
    const checkbox = mount(UiCheckbox, { props: { modelValue: false } })
    await checkbox.get('[role="checkbox"]').trigger('click')
    expect(checkbox.emitted('update:modelValue')?.[0]).toEqual([true])

    const toggle = mount(UiSwitch, { props: { modelValue: false } })
    await toggle.get('button').trigger('click')
    expect(toggle.emitted('update:modelValue')?.[0]).toEqual([true])
  })

  it('分页不会越过首尾页', async () => {
    const wrapper = mount(UiPagination, { props: { modelValue: 1, totalPages: 3 } })
    const buttons = wrapper.findAll('button')
    expect(buttons[0].attributes('disabled')).toBeDefined()
    await buttons[buttons.length - 1].trigger('click')
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([2])
  })

  it.each([
    ['text', 'font-data'],
    ['technical', 'font-data'],
    ['numeric', 'font-data'],
    ['status', 'font-data'],
    ['action', 'font-sans'],
    ['code', 'font-data'],
  ] as const)('表格 %s 内容使用约定字体', (content, expectedClass) => {
    const cell = mount(UiTableCell, { props: { content }, slots: { default: '中文 Latin 123' } })
    expect(cell.attributes('data-content-kind')).toBe(content)
    expect(cell.classes()).toContain(expectedClass)
    expect(cell.classes()).not.toContain('font-mono')
    if (content === 'numeric') expect(cell.classes()).toContain('tabular-nums')
  })

  it('表头使用应用字体而不是数据字体', () => {
    const header = mount(UiTableCell, { props: { as: 'th' }, slots: { default: '服务名' } })
    expect(header.classes()).toContain('font-sans')
    expect(header.classes()).not.toContain('font-data')
  })

  it('业务表格不能绕过统一单元格字体契约', () => {
    const violations = Object.entries(pluginVueSources)
      .filter(([, source]) => /<t[hd](?:\s|>)/.test(source) || /data-cell-|data-table/.test(source))
      .map(([path]) => path)
    expect(violations).toEqual([])
  })

  it('工具页面不能绕过公共组件使用原生表单和表格控件', () => {
    const nativeControls = /<(?:button|input|select|textarea|table)(?:\s|>)/
    const violations = Object.entries(pluginVueSources)
      .filter(([, source]) => nativeControls.test(source))
      .map(([path]) => path)
    expect(violations).toEqual([])
  })

  it('搜索输入框为图标和清空按钮保留固定空间', () => {
    const search = mount(UiSearchInput, { props: { modelValue: '' } })
    expect(search.get('input').classes()).toContain('ui-search-control')
  })

  it('带尾部动作时仍可清空搜索，动作保持独立入口', async () => {
    const search = mount(UiSearchInput, {
      props: { modelValue: 'server' },
      slots: { actions: '<button aria-label="添加服务器">+</button>' },
    })
    await search.get('[aria-label="清空搜索"]').trigger('click')
    expect(search.emitted('update:modelValue')).toEqual([['']])
    expect(search.get('[aria-label="添加服务器"]').exists()).toBe(true)
    search.unmount()
  })
})

/**
 * AR03 补证据：基础 UI 必须能在没有 Pinia、没有凭证库、没有 IPC 的宿主里挂载与交互。
 *
 * 依赖守卫与构建只能证明 import/类型约束，不能证明真的挂得上；这里用无插件宿主真挂，
 * 并顺带断言挂载过程没有「注入缺失」类告警（基础控件不许偷偷依赖应用服务）。
 */
const uiSources = import.meta.glob('./**/*.{vue,ts}', {
  eager: true,
  query: '?raw',
  import: 'default',
}) as Record<string, string>

describe('AR03 补证据：基础 UI 无应用服务可独立挂载', () => {
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('结构复杂控件在没有 Pinia / 凭证库时挂载并响应交互', async () => {
    const warnings: string[] = []
    const warn = vi.spyOn(console, 'warn').mockImplementation((...args: unknown[]) => {
      warnings.push(args.map(String).join(' '))
    })

    // 弹窗：内容经传送门进 body，按用户可见文案与关闭出口断言
    const modal = mount(UiModal, {
      props: { open: false, title: '独立挂载' },
      slots: { default: '正文', footer: '页脚' },
      attachTo: document.body,
    })
    await modal.setProps({ open: true })
    await nextTick()
    const panel = document.body.querySelector('.ui-modal-panel')
    expect(panel?.textContent).toContain('独立挂载')
    expect(panel?.textContent).toContain('正文')
    expect(panel?.textContent).toContain('页脚')
    panel?.querySelector<HTMLButtonElement>('button[title="关闭"]')?.click()
    await nextTick()
    expect(modal.emitted('close')).toBeTruthy()
    modal.unmount()

    // 树：点击节点即派发选中
    const tree = mount(UiTree, {
      props: {
        modelValue: '',
        items: [{ id: 't1', label: '表 A', depth: 0, kind: 'table', expandable: true }],
      },
    })
    await tree.get('[role="treeitem"]').trigger('click')
    expect(tree.emitted('update:modelValue')?.[0]).toEqual(['t1'])
    tree.unmount()

    // 数据表：单元格按列渲染
    const grid = mount(UiDataGrid, {
      props: {
        columns: [
          { key: 'id', label: 'ID' },
          { key: 'name', label: '名称' },
        ],
        rows: [{ id: '1', name: '甲' }],
      },
    })
    expect(grid.text()).toContain('甲')
    grid.unmount()

    // 分栏：分隔条支持键盘调整
    const split = mount(UiSplitPane, {
      props: { modelValue: 240 },
      slots: { primary: 'A', secondary: 'B' },
    })
    await split.get('[role="separator"]').trigger('keydown', { key: 'ArrowRight' })
    expect(split.emitted('update:modelValue')?.[0]?.[0]).toBe(256)
    split.unmount()

    // 可搜索下拉：挂载并回显选中项
    const combo = mount(UiCombobox, {
      props: { modelValue: 'a', options: [{ value: 'a', label: '甲' }] },
    })
    expect(combo.html()).toContain('甲')
    combo.unmount()

    warn.mockRestore()
    expect(warnings.filter((item) => /pinia|inject|provide/i.test(item))).toEqual([])
  })

  it('core/ui 不引入应用服务（stores / 凭证库 / IPC / 平台与反馈层）', () => {
    const forbidden = [
      /from 'pinia'/,
      /from '@\/stores/,
      /from '@\/core\/(?:vault|ipc|platform|feedback)/,
    ]
    const violations = Object.entries(uiSources)
      .filter(([path]) => !path.includes('.test.'))
      .filter(([, source]) => forbidden.some((pattern) => pattern.test(source)))
      .map(([path]) => path)
    expect(violations).toEqual([])
  })
})
