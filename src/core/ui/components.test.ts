import { enableAutoUnmount, mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import UiButton from './UiButton.vue'
import UiInput from './UiInput.vue'
import UiTextarea from './UiTextarea.vue'
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

enableAutoUnmount(afterEach)

const pluginVueSources = import.meta.glob('../../plugins/**/*.vue', {
  eager: true,
  query: '?raw',
  import: 'default',
}) as Record<string, string>

describe('公共 UI 组件', () => {
  it('不可调整的分栏保留内容但退出键盘和拖拽入口', async () => {
    const pane = mount(UiSplitPane, {
      props: { modelValue: 300, resizable: false },
      slots: { primary: '远程文件', secondary: '本地文件' },
    })
    const separator = pane.get('[role="separator"]')
    expect(separator.attributes('tabindex')).toBe('-1')
    expect(separator.attributes('aria-disabled')).toBe('true')
    await separator.trigger('keydown', { key: 'ArrowRight' })
    expect(pane.emitted('update:modelValue')).toBeUndefined()
    expect(pane.text()).toContain('本地文件')
  })

  it('文本、密码、多行和组合框统一关闭浏览器表单自动填充', () => {
    for (const type of ['text', 'number', 'password']) {
      const wrapper = mount(UiInput, { props: { type }, attrs: { autocomplete: 'on' } })
      expect(wrapper.get('input').attributes('autocomplete')).toBe('off')
    }
    const textarea = mount(UiTextarea, { attrs: { autocomplete: 'on' } })
    expect(textarea.get('textarea').attributes('autocomplete')).toBe('off')
    const combobox = mount(UiCombobox, { props: { modelValue: '', options: [] } })
    expect(combobox.get('input').attributes('autocomplete')).toBe('off')
  })

  it('按钮在加载中自动禁用并显示加载状态', () => {
    const wrapper = mount(UiButton, { props: { loading: true }, slots: { default: '保存' } })
    expect(wrapper.get('button').attributes('disabled')).toBeDefined()
    expect(wrapper.find('.animate-spin').exists()).toBe(true)
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

  it('页签触发器渲染为 div（关闭按钮须是独立 button，禁止 button 嵌套）', () => {
    const wrapper = mount(UiTabs, {
      props: {
        modelValue: 'a',
        items: [{ value: 'a', label: '页签 A', closable: true }],
      },
    })
    const trigger = wrapper.get('[role="tab"]')
    expect(trigger.element.tagName).toBe('DIV')
    const close = trigger.get('[aria-label="关闭页签 A"]')
    expect(close.element.tagName).toBe('BUTTON')
  })

  it('UiModal full 档：header 固定、内容弹性、Esc 触发 close', async () => {
    const wrapper = mount(UiModal, {
      attachTo: document.body,
      props: { open: true, size: 'full', width: '800px', title: '编辑器' },
      slots: { default: '<div class="content">主体</div>' },
    })
    // DialogPortal 挂到 body，等一拍让 portal 完成渲染
    await nextTick()
    const panel = document.body.querySelector<HTMLElement>('.ui-modal-panel')
    expect(panel).toBeTruthy()
    // full 档：最大宽度生效、高度不受 85vh 限制（!max-h-none）
    expect(panel!.style.maxWidth).toBe('800px')
    expect(panel!.className).toContain('flex-col')
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await nextTick()
    expect(wrapper.emitted('close')).toBeTruthy()
    wrapper.unmount()
  })

  it('UiModal 非 full 档：header/footer 固定在滚动区外（面板本身不滚）', async () => {
    const wrapper = mount(UiModal, {
      attachTo: document.body,
      props: { open: true, title: '设置', size: 'md' },
      slots: { default: '<p>很长很长的内容</p>', footer: '<button>确定</button>' },
    })
    await nextTick()
    const panel = document.body.querySelector<HTMLElement>('.ui-modal-panel')
    expect(panel).toBeTruthy()
    const scrollArea = panel!.querySelector('[data-scroll-axis="vertical"]')
    expect(scrollArea).toBeTruthy()
    // header 与 footer 是面板的直接子级，不在滚动容器内（滚动只发生在内容区）
    const header = panel!.querySelector('header')
    const footer = panel!.querySelector('footer')
    expect(header).toBeTruthy()
    expect(footer).toBeTruthy()
    expect(scrollArea!.contains(header)).toBe(false)
    expect(scrollArea!.contains(footer)).toBe(false)
    expect(scrollArea!.textContent).toContain('很长很长的内容')
    wrapper.unmount()
  })

  it('基础控件输出统一尺寸类', () => {
    expect(
      mount(UiButton, { props: { size: 'xs' } })
        .get('button')
        .classes()
    ).toContain('ui-control-xs')
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
      .filter(([path, source]) => {
        // 用户明确指定 SSH 页签行编辑器入口使用原生文字按钮，仅放行这一个入口。
        const checked =
          path === '../../plugins/ssh/index.vue'
            ? source.replace(
                /<button\b(?=[^>]*\bdata-ssh-editor-action)[^>]*>[\s\S]*?<\/button>/,
                ''
              )
            : source
        return nativeControls.test(checked)
      })
      .map(([path]) => path)
    expect(violations).toEqual([])
  })

  it('带尾部动作时仍可清空搜索，动作保持独立入口', async () => {
    const search = mount(UiSearchInput, {
      props: { modelValue: 'server' },
      slots: { actions: '<button aria-label="添加服务器">+</button>' },
    })
    await search.get('[aria-label="清空搜索"]').trigger('click')
    expect(search.emitted('update:modelValue')).toEqual([['']])
    expect(search.find('[aria-label="添加服务器"]').exists()).toBe(true)
    search.unmount()
  })
})

/**
 * AR03 补证据：基础 UI 必须能在没有 Pinia、没有凭证库、没有 IPC 的宿主里挂载与交互。
 *
 * 依赖守卫与构建只能证明 import/类型约束，不能证明真的挂得上；这里用无插件宿主真挂，
 * 并顺带断言挂载过程没有「注入缺失」类告警（基础控件不许偷偷依赖应用服务）。
 */
describe('基础 UI 无应用服务可独立挂载', () => {
  afterEach(() => {
    vi.restoreAllMocks()
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
    const close = panel?.querySelector<HTMLButtonElement>('button[aria-label="关闭"]')
    expect(close).toBeTruthy()
    close!.click()
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

    // 树：roving tabindex + 方向键导航（↓ 移焦点，→ 展开可展开项）
    const treeKb = mount(UiTree, {
      attachTo: document.body,
      props: {
        modelValue: '',
        items: [
          { id: 'p1', label: '父一', depth: 0, expandable: true, expanded: false },
          { id: 'p2', label: '父二', depth: 0, expandable: false },
        ],
      },
    })
    const treeItems = treeKb.findAll('[role="treeitem"]')
    ;(treeItems[0].element as HTMLElement).focus()
    await treeItems[0].trigger('keydown', { key: 'ArrowDown' })
    expect(document.activeElement).toBe(treeItems[1].element)
    await treeItems[1].trigger('keydown', { key: 'ArrowUp' })
    expect(document.activeElement).toBe(treeItems[0].element)
    await treeItems[0].trigger('keydown', { key: 'ArrowRight' })
    expect(treeKb.emitted('toggle')?.[0]?.[0]).toMatchObject({ id: 'p1' })
    treeKb.unmount()

    // 数据表：content 语义落地（action 列 sans、numeric 列等宽数字、默认数据字体）
    const gridContent = mount(UiDataGrid, {
      props: {
        rowNumbers: false,
        columns: [
          { key: 'name', label: '名称', content: 'technical' },
          { key: 'count', label: '数量', content: 'numeric' },
          { key: 'ops', label: '操作', content: 'action' },
        ],
        rows: [{ name: '甲', count: 3, ops: 'x' }],
      },
    })
    const tds = gridContent.findAll('tbody td')
    expect(tds[0].classes()).toContain('font-data')
    expect(tds[1].classes()).toContain('tabular-nums')
    expect(tds[2].classes()).toContain('font-sans')
    gridContent.unmount()

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
})
