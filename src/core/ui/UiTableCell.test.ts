/**
 * UiTableCell 契约测试：表头不换行、resizable 拖拽调宽与双击复位。
 */
import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import UiTableCell from './UiTableCell.vue'

describe('UiTableCell 表头行为', () => {
  it('th 默认不换行', () => {
    const wrapper = mount(UiTableCell, {
      props: { as: 'th' },
      slots: { default: '名称' },
    })
    expect(wrapper.classes()).toContain('whitespace-nowrap')
  })

  it('td 不受影响（不换行只加在 th 上）', () => {
    const wrapper = mount(UiTableCell, {
      props: { as: 'td' },
      slots: { default: '内容' },
    })
    expect(wrapper.classes()).not.toContain('whitespace-nowrap')
  })

  it('resizable 表头渲染拖拽手柄，拖拽写入行内宽度并有下限', async () => {
    // resizable 默认开启：不传 prop 也应有手柄
    const wrapper = mount(UiTableCell, {
      props: { as: 'th' },
      slots: { default: '名称' },
      attachTo: document.body,
    })
    const handle = wrapper.find('[aria-hidden="true"]')
    expect(handle.exists()).toBe(true)
    expect(handle.classes()).toContain('cursor-col-resize')

    const cell = wrapper.element as HTMLElement
    // jsdom 无布局，getBoundingClientRect 恒 0：从 0 起拖 80px 应写入下限 40px 逻辑
    await handle.trigger('pointerdown', { clientX: 100 })
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 180 }))
    expect(cell.style.minWidth).toBe('80px')
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 10 }))
    expect(cell.style.minWidth).toBe('40px')
    window.dispatchEvent(new PointerEvent('pointerup'))
    expect(document.body.classList.contains('ui-drag-select-lock')).toBe(false)
    wrapper.unmount()
  })

  it('双击手柄复位自动宽度', async () => {
    const wrapper = mount(UiTableCell, {
      props: { as: 'th', resizable: true },
      slots: { default: '名称' },
      attachTo: document.body,
    })
    const handle = wrapper.find('[aria-hidden="true"]')
    const cell = wrapper.element as HTMLElement
    await handle.trigger('pointerdown', { clientX: 100 })
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 160 }))
    window.dispatchEvent(new PointerEvent('pointerup'))
    expect(cell.style.width).toBe('60px')

    await handle.trigger('dblclick')
    expect(cell.style.width).toBe('')
    expect(cell.style.minWidth).toBe('')
    wrapper.unmount()
  })

  it('td 即使传 resizable 也不渲染手柄', () => {
    const wrapper = mount(UiTableCell, {
      props: { as: 'td', resizable: true },
      slots: { default: '内容' },
    })
    expect(wrapper.find('[aria-hidden="true"]').exists()).toBe(false)
  })

  it('显式关闭后不渲染手柄', () => {
    const wrapper = mount(UiTableCell, {
      props: { as: 'th', resizable: false },
      slots: { default: '名称' },
    })
    expect(wrapper.find('[aria-hidden="true"]').exists()).toBe(false)
  })
})
