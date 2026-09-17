import { mount } from '@vue/test-utils'
import { defineComponent, nextTick, ref } from 'vue'
import { expect, it, vi } from 'vitest'
import UiScrollArea from './UiScrollArea.vue'

it('提供横向、纵向和双向入口并保留原语义元素', async () => {
  const wrapper = mount(UiScrollArea, {
    props: { axis: 'horizontal', as: 'nav' },
    slots: { default: '内容' },
  })
  expect(wrapper.element.tagName).toBe('NAV')
  expect(wrapper.attributes('data-scroll-axis')).toBe('horizontal')
  await wrapper.setProps({ axis: 'both' })
  expect(wrapper.attributes('data-scroll-axis')).toBe('both')
  await wrapper.setProps({ axis: 'vertical' })
  expect(wrapper.attributes('data-scroll-axis')).toBe('vertical')
  wrapper.unmount()
})

it('as-child 保留原生 ref、位置读写、滚动事件和拖拽事件', async () => {
  const viewport = ref<HTMLDivElement | null>(null)
  const scroll = vi.fn()
  const pointer = vi.fn()
  const theme = ref<'auto' | 'dark'>('auto')
  const wrapper = mount(
    defineComponent({
      components: { UiScrollArea },
      setup: () => ({ viewport, scroll, pointer, theme }),
      template:
        '<UiScrollArea as-child axis="both" :theme="theme"><div ref="viewport" tabindex="0" @scroll="scroll" @pointerdown="pointer"><span>内容</span></div></UiScrollArea>',
    })
  )
  const element = wrapper.get('div').element
  expect(viewport.value).toBe(element)
  element.scrollTop = 120
  element.scrollLeft = 48
  await wrapper.get('div').trigger('scroll')
  await wrapper.get('div').trigger('pointerdown', { button: 0 })
  expect(scroll).toHaveBeenCalledTimes(1)
  expect(pointer).toHaveBeenCalledTimes(1)
  theme.value = 'dark'
  await nextTick()
  expect(viewport.value).toBe(element)
  expect(element.scrollTop).toBe(120)
  expect(element.scrollLeft).toBe(48)
  expect(wrapper.findAll('div')).toHaveLength(1)
  wrapper.unmount()
  expect(viewport.value).toBeNull()
})
