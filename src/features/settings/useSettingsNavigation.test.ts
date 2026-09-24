/* eslint-disable vue/one-component-per-file -- 各用例独立挂载组件，隔离滚动容器与生命周期。 */
import { mount } from '@vue/test-utils'
import { computed, defineComponent, h, nextTick, ref } from 'vue'
import UiScrollArea from '@/core/ui/UiScrollArea.vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useSettingsNavigation } from './useSettingsNavigation'

afterEach(() => vi.unstubAllGlobals())

describe('设置目录跟随滚动', () => {
  it('公共滚动组件在高亮重绘后仍使用同一滚动元素', async () => {
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        disconnect() {}
      }
    )
    let navigation!: ReturnType<typeof useSettingsNavigation>
    const wrapper = mount(
      defineComponent({
        setup() {
          const area = ref<InstanceType<typeof UiScrollArea> | null>(null)
          navigation = useSettingsNavigation(computed(() => area.value?.$el ?? null))
          return () =>
            h(UiScrollArea, { ref: area }, () => [
              h('section', { 'data-settings-section': 'appearance' }),
              h('section', { 'data-settings-section': 'about' }, navigation.activeSection.value),
            ])
        },
      })
    )
    const root = wrapper.element as HTMLElement
    Object.defineProperties(root, { clientHeight: { value: 400 }, scrollHeight: { value: 1000 } })
    root.scrollTop = 600
    root.dispatchEvent(new Event('scroll'))
    await nextTick()
    expect(navigation.activeSection.value).toBe('about')
    root.scrollTop = 0
    root.children[1].getBoundingClientRect = () => ({ top: 700 }) as DOMRect
    root.dispatchEvent(new Event('scroll'))
    await nextTick()
    expect(navigation.activeSection.value).toBe('appearance')
    expect(wrapper.element).toBe(root)
    wrapper.unmount()
  })
  it('跟随分区、页底和内容尺寸变化，并在卸载时释放观察器', () => {
    let resize = () => {}
    const disconnect = vi.fn()
    vi.stubGlobal(
      'ResizeObserver',
      class {
        constructor(callback: () => void) {
          resize = callback
        }
        observe() {}
        disconnect = disconnect
      }
    )
    const root = document.createElement('div')
    root.innerHTML =
      '<div><section data-settings-section="appearance"></section><section data-settings-section="general"></section><section data-settings-section="about"></section></div>'
    Object.defineProperties(root, { clientHeight: { value: 400 }, scrollHeight: { value: 1200 } })
    const tops = [0, 300, 1000]
    root.querySelectorAll('section').forEach((section, index) => {
      section.getBoundingClientRect = () => ({ top: tops[index] - root.scrollTop }) as DOMRect
    })
    let navigation!: ReturnType<typeof useSettingsNavigation>
    const wrapper = mount(
      defineComponent({
        setup() {
          navigation = useSettingsNavigation(ref(root))
          return () => h('div')
        },
      })
    )
    expect(navigation.activeSection.value).toBe('appearance')
    root.scrollTop = 320
    root.dispatchEvent(new Event('scroll'))
    expect(navigation.activeSection.value).toBe('general')
    root.scrollTop = 800
    root.dispatchEvent(new Event('scroll'))
    expect(navigation.activeSection.value).toBe('about')
    root.scrollTop = 320
    tops[1] = 600
    resize()
    expect(navigation.activeSection.value).toBe('appearance')
    wrapper.unmount()
    expect(disconnect).toHaveBeenCalledOnce()
    root.scrollTop = 800
    root.dispatchEvent(new Event('scroll'))
    expect(navigation.activeSection.value).toBe('appearance')
  })

  it('点击按滚动容器定位，并尊重减少动画偏好', () => {
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        disconnect() {}
      }
    )
    const media = vi
      .spyOn(window, 'matchMedia')
      .mockReturnValue({ matches: true } as MediaQueryList)
    const root = document.createElement('div')
    root.innerHTML = '<section data-settings-section="about"></section>'
    root.scrollTop = 100
    root.getBoundingClientRect = () => ({ top: 36 }) as DOMRect
    root.firstElementChild!.getBoundingClientRect = () => ({ top: 500 }) as DOMRect
    root.scrollTo = vi.fn()
    let navigation!: ReturnType<typeof useSettingsNavigation>
    const wrapper = mount(
      defineComponent({
        setup() {
          navigation = useSettingsNavigation(ref(root))
          return () => h('div')
        },
      })
    )
    navigation.goToSection('about')
    expect(root.scrollTo).toHaveBeenCalledWith({ top: 550, behavior: 'instant' })
    navigation.goToSection('missing')
    expect(root.scrollTo).toHaveBeenCalledOnce()
    wrapper.unmount()
    media.mockRestore()
  })
})
