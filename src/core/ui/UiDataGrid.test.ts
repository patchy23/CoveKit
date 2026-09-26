import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import UiDataGrid from './UiDataGrid.vue'

enableAutoUnmount(afterEach)
afterEach(() => vi.restoreAllMocks())

const columns = [
  { key: 'a', label: 'A', width: 100 },
  { key: 'b', label: 'B', width: 120 },
]
const move = () => window.dispatchEvent(new PointerEvent('pointermove', { clientX: 80 }))

it('保留仍展示列的用户宽度，移除列后不再持有其历史宽度', async () => {
  const grid = mount(UiDataGrid, { props: { columns, rows: [], rowNumbers: false } })
  await grid.get('th span.cursor-col-resize').trigger('pointerdown', { clientX: 20 })
  move()
  window.dispatchEvent(new PointerEvent('pointerup'))
  await grid.vm.$nextTick()
  expect((grid.get('th').element as HTMLElement).style.width).toBe('160px')
  await grid.setProps({ columns: [{ key: 'a', label: '新标题', width: 200 }] })
  expect((grid.get('th').element as HTMLElement).style.width).toBe('160px')
  await grid.setProps({ columns: [{ key: 'b', label: 'B', width: 180 }] })
  expect((grid.get('th').element as HTMLElement).style.width).toBe('180px')
  await grid.setProps({ columns })
  expect((grid.get('th').element as HTMLElement).style.width).toBe('100px')
})

for (const reason of ['pointercancel', 'remove-column', 'unmount']) {
  it(`拖动在 ${reason} 时释放监听器且后续移动不再修改宽度`, async () => {
    const grid = mount(UiDataGrid, { props: { columns, rows: [], rowNumbers: false } })
    const remove = vi.spyOn(window, 'removeEventListener')
    await grid.get('th span.cursor-col-resize').trigger('pointerdown', { clientX: 20 })
    if (reason === 'unmount') grid.unmount()
    else if (reason === 'remove-column') await grid.setProps({ columns: [columns[1]] })
    else window.dispatchEvent(new PointerEvent('pointercancel'))
    for (const name of ['pointermove', 'pointerup', 'pointercancel', 'blur']) {
      expect(remove).toHaveBeenCalledWith(name, expect.any(Function))
    }
    move()
    if (reason !== 'unmount') {
      await grid.vm.$nextTick()
      expect((grid.get('th').element as HTMLElement).style.width).toBe(
        reason === 'remove-column' ? '120px' : '100px'
      )
    }
  })
}
