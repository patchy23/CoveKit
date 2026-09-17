import { mount } from '@vue/test-utils'
import { defineComponent, nextTick, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import UiButton from './UiButton.vue'
import UiIconButton from './UiIconButton.vue'
import UiTooltip from './UiTooltip.vue'
import UiDataGrid from './UiDataGrid.vue'
import UiTabsOverflow from './UiTabsOverflow.vue'

const wrappers: Array<{ unmount: () => void }> = []
afterEach(() => {
  wrappers.splice(0).forEach((wrapper) => wrapper.unmount())
  vi.restoreAllMocks()
  vi.useRealTimers()
  document.body.innerHTML = ''
})

it('悬停延迟显示纯文本，禁用后关闭且卸载清理浮层', async () => {
  vi.useFakeTimers()
  const wrapper = mount(UiTooltip, {
    attachTo: document.body,
    props: { content: '<b>提示</b>' },
    slots: { default: '<button>操作</button>' },
  })
  wrappers.push(wrapper)
  await wrapper.get('button').trigger('pointermove', { pointerType: 'mouse' })
  await vi.advanceTimersByTimeAsync(399)
  expect(document.querySelector('.ui-tooltip')).toBeNull()
  await vi.advanceTimersByTimeAsync(1)
  await nextTick()
  expect(document.querySelector('.ui-tooltip')?.textContent).toContain('<b>提示</b>')
  expect(document.querySelector('.ui-tooltip b')).toBeNull()
  await wrapper.setProps({ disabled: true })
  expect(document.querySelector('.ui-tooltip')).toBeNull()
  wrapper.unmount()
  wrappers.pop()
  expect(document.querySelector('[role="tooltip"]')).toBeNull()
})

it('按钮保留点击、尺寸、表单类型和无障碍标签，不输出原生 title', async () => {
  const click = vi.fn()
  const wrapper = mount(UiIconButton, {
    attachTo: document.body,
    props: { label: '添加连接', size: 'xs' },
    attrs: { onClick: click, title: '新建 SSH 连接', type: 'submit' },
  })
  wrappers.push(wrapper)
  const button = wrapper.get('button')
  expect(wrapper.findAll('button')).toHaveLength(1)
  expect(button.attributes('title')).toBeUndefined()
  expect(button.attributes('aria-label')).toBe('添加连接')
  expect(button.attributes('type')).toBe('submit')
  expect(button.classes()).toContain('ui-control-xs')
  await button.trigger('click')
  expect(click).toHaveBeenCalledTimes(1)
})

it('快速划过多个按钮时取消旧提示，停留后始终只显示当前一个', async () => {
  vi.useFakeTimers()
  const wrapper = mount(
    {
      components: { UiTooltip },
      template: `<div><UiTooltip v-for="name in ['最小化', '最大化', '关闭']" :key="name" :content="name"><button>{{ name }}</button></UiTooltip></div>`,
    },
    { attachTo: document.body }
  )
  wrappers.push(wrapper)
  const buttons = wrapper.findAll('button')
  await buttons[0].trigger('pointermove', { buttons: 0, pointerType: 'mouse' })
  await vi.advanceTimersByTimeAsync(400)
  expect(document.querySelectorAll('.ui-tooltip')).toHaveLength(1)
  await buttons[0].trigger('pointerleave')
  expect(document.querySelector('.ui-tooltip')).toBeNull()
  await buttons[1].trigger('pointermove', { buttons: 0, pointerType: 'mouse' })
  await vi.advanceTimersByTimeAsync(100)
  // 即使旧目标的离开事件迟到，新目标也立即取得唯一归属。
  await buttons[2].trigger('pointermove', { buttons: 0, pointerType: 'mouse' })
  await vi.advanceTimersByTimeAsync(300)
  expect(document.querySelector('.ui-tooltip')).toBeNull()
  await vi.advanceTimersByTimeAsync(100)
  const tips = document.querySelectorAll('.ui-tooltip')
  expect(tips).toHaveLength(1)
  expect(tips[0].textContent).toContain('关闭')
  expect(tips[0].classList.contains('!pointer-events-none')).toBe(true)
  expect(tips[0].classList.contains('select-none')).toBe(true)
  await buttons[2].trigger('pointerleave')
  await vi.advanceTimersByTimeAsync(1000)
  expect(document.querySelector('.ui-tooltip')).toBeNull()
})

it('键盘聚焦显示提示并建立描述关联，Esc 关闭', async () => {
  const wrapper = mount(UiButton, {
    attachTo: document.body,
    props: { title: '保存修改' },
    slots: { default: '保存' },
  })
  wrappers.push(wrapper)
  const button = wrapper.get('button')
  vi.spyOn(button.element, 'matches').mockImplementation(
    (selector) => selector === ':focus-visible'
  )
  await button.trigger('focus')
  const description = button.attributes('aria-describedby')
  expect(description).toBeTruthy()
  expect(document.getElementById(description)?.textContent).toContain('保存修改')
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  await nextTick()
  expect(document.querySelector('.ui-tooltip')).toBeNull()
})

it('空提示保持单一按钮且不渲染浮层', () => {
  const wrapper = mount(UiButton, { props: { loading: true }, slots: { default: '保存' } })
  wrappers.push(wrapper)
  expect(wrapper.findAll('button')).toHaveLength(1)
  expect(wrapper.get('button').attributes('disabled')).toBeDefined()
  expect(wrapper.find('.ui-spinner').exists()).toBe(true)
  expect(wrapper.find('.ui-tooltip').exists()).toBe(false)
})

it('提示文字在空值间变化时保留触发元素、焦点与原始 ref', async () => {
  const target = ref<HTMLButtonElement | null>(null)
  const content = ref('路径')
  const wrapper = mount(
    defineComponent({
      components: { UiTooltip },
      setup: () => ({ target, content }),
      template: '<UiTooltip :content="content"><button ref="target">路径</button></UiTooltip>',
    }),
    { attachTo: document.body }
  )
  wrappers.push(wrapper)
  const element = wrapper.get('button').element
  expect(target.value).toBe(element)
  element.focus()
  content.value = ''
  await nextTick()
  expect(wrapper.get('button').element).toBe(element)
  expect(target.value).toBe(element)
  expect(document.activeElement).toBe(element)
  content.value = '新路径'
  await nextTick()
  expect(target.value).toBe(element)
  wrapper.unmount()
  wrappers.pop()
  expect(target.value).toBeNull()
})

it('按住鼠标的拖拽移动继续到达窗口监听', async () => {
  const move = vi.fn()
  window.addEventListener('pointermove', move)
  const wrapper = mount(UiTooltip, {
    attachTo: document.body,
    props: { content: '拖拽调整宽度' },
    slots: { default: '<div role="separator" />' },
  })
  wrappers.push(wrapper)
  try {
    await wrapper
      .get('[role="separator"]')
      .trigger('pointermove', { buttons: 1, pointerType: 'mouse' })
    expect(move).toHaveBeenCalledTimes(1)
    await wrapper
      .get('[role="separator"]')
      .trigger('pointermove', { buttons: 0, pointerType: 'mouse' })
    expect(move).toHaveBeenCalledTimes(1)
  } finally {
    window.removeEventListener('pointermove', move)
  }
})

it('表格提示不改变单元格父节点或双击载荷，循环更新保持对应关系', async () => {
  const row = { name: '主机', status: '在线' }
  const columns = [
    { key: 'name', label: '名称' },
    { key: 'status', label: '状态' },
  ]
  const wrapper = mount(UiDataGrid, { props: { rows: [row], columns, rowNumbers: false } })
  wrappers.push(wrapper)
  const cell = wrapper.get('tbody td')
  expect(cell.element.parentElement?.tagName).toBe('TR')
  expect(wrapper.findAll('tbody td')).toHaveLength(2)
  await cell.trigger('dblclick')
  expect(wrapper.emitted('cell')?.[0]).toEqual([{ row, column: columns[0] }])
  await wrapper.setProps({ columns: [columns[1]] })
  expect(wrapper.findAll('tbody td')).toHaveLength(1)
  expect(wrapper.get('tbody td').text()).toBe('在线')
})

it('页签溢出按钮仍通过原始元素引用定位菜单', async () => {
  const wrapper = mount(UiTabsOverflow, {
    attachTo: document.body,
    props: { items: [{ value: 'a', label: '连接 A' }] },
  })
  wrappers.push(wrapper)
  const button = wrapper.get('button')
  vi.spyOn(button.element, 'getBoundingClientRect').mockReturnValue({
    bottom: 64,
    right: 300,
  } as DOMRect)
  await button.trigger('click')
  const panel = document.body.querySelector<HTMLElement>('.fixed.z-\\[220\\]')
  expect(panel?.style.top).toBe('68px')
  expect(panel?.style.left).toBe('80px')
})
