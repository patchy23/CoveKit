import { mount } from '@vue/test-utils'
import { defineComponent } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { useSplitPane } from './useSplitPane'

afterEach(() => vi.restoreAllMocks())

function setup() {
  let pane!: ReturnType<typeof useSplitPane>
  const wrapper = mount(
    defineComponent({
      setup() {
        pane = useSplitPane({ initial: 200, min: 100, max: 400 })
        return () => null
      },
    })
  )
  return { wrapper, pane }
}

it('拖动中卸载会移除全部全局监听并释放动态边界回调', () => {
  const add = vi.spyOn(window, 'addEventListener')
  const remove = vi.spyOn(window, 'removeEventListener')
  const { wrapper, pane } = setup()
  const getMax = vi.fn(() => 350)
  pane.onPointerDown(new MouseEvent('mousedown', { clientX: 100 }), getMax)
  window.dispatchEvent(new MouseEvent('mousemove', { clientX: 120 }))
  expect(pane.size.value).toBe(220)
  wrapper.unmount()
  for (const [type, handler] of add.mock.calls.filter(([type]) =>
    ['mousemove', 'mouseup', 'blur'].includes(type)
  )) {
    expect(remove).toHaveBeenCalledWith(type, handler)
  }
  getMax.mockClear()
  window.dispatchEvent(new MouseEvent('mousemove', { clientX: 140 }))
  expect(getMax).not.toHaveBeenCalled()
  expect(pane.size.value).toBe(220)
})

it('失焦或重复开始拖动不会留下旧监听，正常鼠标释放也结束拖动', () => {
  const { wrapper, pane } = setup()
  const oldMax = vi.fn(() => 400)
  const currentMax = vi.fn(() => 350)
  pane.onPointerDown(new MouseEvent('mousedown'), oldMax)
  pane.onPointerDown(new MouseEvent('mousedown'), currentMax)
  window.dispatchEvent(new MouseEvent('mousemove', { clientX: 20 }))
  expect(oldMax).not.toHaveBeenCalled()
  expect(currentMax).toHaveBeenCalledTimes(1)
  window.dispatchEvent(new Event('blur'))
  window.dispatchEvent(new MouseEvent('mousemove', { clientX: 40 }))
  expect(currentMax).toHaveBeenCalledTimes(1)
  pane.onPointerDown(new MouseEvent('mousedown'), currentMax)
  window.dispatchEvent(new MouseEvent('mouseup'))
  window.dispatchEvent(new MouseEvent('mousemove'))
  expect(currentMax).toHaveBeenCalledTimes(1)
  wrapper.unmount()
})
