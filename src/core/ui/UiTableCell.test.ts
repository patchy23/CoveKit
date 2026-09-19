/**
 * UiTableCell 契约测试：表头不换行、resizable 拖拽调宽与双击复位。
 */
import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import UiTableCell from './UiTableCell.vue'

describe('UiTableCell 表头行为', () => {
  it('fit 模式仅在相邻数据列间分配比例，保留固定操作列，双击恢复默认', async () => {
    const wrapper = mount({
      components: { UiTableCell },
      template:
        '<table><thead><tr><UiTableCell as="th" resize-mode="fit">名称</UiTableCell><UiTableCell as="th" resize-mode="fit">路径</UiTableCell><UiTableCell as="th" :resizable="false">操作</UiTableCell></tr></thead></table>',
    })
    const headers = wrapper.findAll('th')
    const widths = [100, 200, 288]
    headers.forEach((header, index) =>
      vi
        .spyOn(header.element, 'getBoundingClientRect')
        .mockReturnValue({ width: widths[index] } as DOMRect)
    )
    try {
      const handle = headers[0].get('.cursor-col-resize')
      await handle.trigger('pointerdown', { clientX: 100 })
      window.dispatchEvent(new PointerEvent('pointermove', { clientX: 150 }))
      expect((headers[0].element as HTMLElement).style.width).toBe('calc(50% - 144px)')
      expect((headers[1].element as HTMLElement).style.width).toBe('calc(50% - 144px)')
      expect((headers[2].element as HTMLElement).style.width).toBe('')
      expect((headers[0].element as HTMLElement).style.minWidth).toBe('')
      window.dispatchEvent(new PointerEvent('pointermove', { clientX: 1000 }))
      expect((headers[1].element as HTMLElement).style.width).toContain(`${(40 / 300) * 100}%`)
      window.dispatchEvent(new PointerEvent('pointerup'))
      await handle.trigger('dblclick')
      headers.forEach((header) => expect((header.element as HTMLElement).style.width).toBe(''))
    } finally {
      window.dispatchEvent(new PointerEvent('pointerup'))
      wrapper.unmount()
      vi.restoreAllMocks()
    }
  })
  for (const reason of ['pointercancel', 'blur', 'unmount', 'disabled']) {
    it(`拖拽在 ${reason} 后释放全局锁并停止修改列宽`, async () => {
      const wrapper = mount(UiTableCell, { props: { as: 'th' }, attachTo: document.body })
      const cell = wrapper.element as HTMLElement
      try {
        await wrapper.find('[aria-hidden="true"]').trigger('pointerdown', { clientX: 100 })
        window.dispatchEvent(new PointerEvent('pointermove', { clientX: 180 }))
        const width = cell.style.width
        expect(document.body.classList.contains('ui-drag-select-lock')).toBe(true)
        if (reason === 'unmount') wrapper.unmount()
        else if (reason === 'disabled') await wrapper.setProps({ resizable: false })
        else window.dispatchEvent(new Event(reason))
        expect(document.body.classList.contains('ui-drag-select-lock')).toBe(false)
        window.dispatchEvent(new PointerEvent('pointermove', { clientX: 250 }))
        expect(cell.style.width).toBe(width)
      } finally {
        window.dispatchEvent(new PointerEvent('pointerup'))
        wrapper.unmount()
      }
    })
  }

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
