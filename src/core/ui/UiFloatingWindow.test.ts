import { defineComponent, h, nextTick } from 'vue'
import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import UiFloatingWindow from './UiFloatingWindow.vue'

enableAutoUnmount(afterEach)
afterEach(() => vi.restoreAllMocks())
it('拖动、缩放限制在宿主范围；最大化可恢复且激活不会重建内容', async () => {
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(1000)
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(700)
  const wrapper = mount(
    defineComponent({
      render: () =>
        h('div', [
          h(UiFloatingWindow, { title: 'A' }, () => h('p', '日志')),
          h(UiFloatingWindow, { title: 'B' }),
        ]),
    })
  )
  await nextTick()
  const [first, second] = wrapper.findAllComponents(UiFloatingWindow)
  const panel = first.get('section').element
  const content = first.get('p').element
  expect(Number(panel.style.zIndex)).toBeLessThan(
    Number(second.get('section').element.style.zIndex)
  )
  await first
    .get('header')
    .trigger('pointerdown', { button: 0, pointerId: 1, clientX: 10, clientY: 10 })
  window.dispatchEvent(
    new PointerEvent('pointermove', { pointerId: 1, clientX: 4000, clientY: 4000 })
  )
  window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 1 }))
  await nextTick()
  expect(panel.style.left).toBe('180px')
  expect(panel.style.top).toBe('180px')
  expect(Number(panel.style.zIndex)).toBeGreaterThan(
    Number(second.get('section').element.style.zIndex)
  )
  expect(Number(panel.style.zIndex)).toBeLessThan(180)
  await first.get('[aria-label="最大化窗口"]').trigger('click')
  expect(panel.style.width).toBe('1000px')
  await first.get('[aria-label="还原窗口"]').trigger('click')
  expect(panel.style.width).toBe('820px')
  expect(first.get('p').element).toBe(content)
  await first
    .get('[aria-label="调整窗口大小，方向键调整"]')
    .trigger('keydown', { key: 'ArrowLeft' })
  expect(panel.style.width).toBe('810px')
  await first
    .get('[aria-label="调整窗口大小，方向键调整"]')
    .trigger('pointerdown', { button: 0, pointerId: 2, clientX: 0, clientY: 0 })
  const remove = vi.spyOn(window, 'removeEventListener')
  wrapper.unmount()
  expect(remove).toHaveBeenCalledWith('pointermove', expect.any(Function))
  expect(remove).toHaveBeenCalledWith('pointercancel', expect.any(Function))
})
