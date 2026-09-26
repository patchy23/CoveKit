import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import Tts from './index.vue'
import type { TtsResult } from './contracts'

const mocks = vi.hoisted(() => ({ synthesize: vi.fn(), voices: vi.fn(), toast: vi.fn() }))
vi.mock('./ipc', () => ({ ipc: { ttsSynthesize: mocks.synthesize, ttsVoices: mocks.voices } }))
vi.mock('@tauri-apps/api/core', () => ({ convertFileSrc: (path: string) => `asset:${path}` }))
vi.mock('@/stores/ui', () => ({ useUiStore: () => ({ toast: mocks.toast }) }))
vi.mock('@/features/ui/AppIcon.vue', () => ({ default: { template: '<i />' } }))
vi.mock('@/core/ui', () => ({
  UiButton: { template: '<button><slot /></button>' },
  UiEmptyState: { template: '<div><slot /></div>' },
  UiField: { template: '<div><slot /></div>' },
  UiToolbar: { template: '<header><slot /></header>' },
  UiSlider: { template: '<input />' },
  UiSelect: { template: '<select />' },
  UiTextarea: { template: '<textarea />' },
}))
enableAutoUnmount(afterEach)
beforeEach(() => {
  vi.clearAllMocks()
  mocks.voices.mockResolvedValue([])
  mocks.synthesize.mockResolvedValue({ ok: true, filePath: '/test.mp3', bytes: 123 })
})
afterEach(() => vi.restoreAllMocks())

function button(wrapper: ReturnType<typeof mount>, label: string) {
  return wrapper.findAll('button').find((item) => item.text() === label)!
}

it.each(['clear', 'close'])('%s 时停止播放并卸载音频源', async (action) => {
  const wrapper = mount(Tts)
  await button(wrapper, '合成语音').trigger('click')
  await flushPromises()
  const audio = wrapper.get('audio').element as HTMLAudioElement
  const pause = vi.spyOn(audio, 'pause').mockImplementation(() => {})
  const load = vi.spyOn(audio, 'load').mockImplementation(() => {})
  if (action === 'clear') await button(wrapper, '清空').trigger('click')
  else wrapper.unmount()
  expect(pause).toHaveBeenCalledOnce()
  expect(load).toHaveBeenCalledOnce()
  expect(audio.hasAttribute('src')).toBe(false)
})

it.each(['clear', 'close'])('%s 后迟到的合成结果不恢复播放器或弹成功提示', async (action) => {
  let finish!: (result: TtsResult) => void
  mocks.synthesize.mockReturnValueOnce(new Promise<TtsResult>((resolve) => (finish = resolve)))
  const wrapper = mount(Tts)
  await button(wrapper, '合成语音').trigger('click')
  await button(wrapper, '合成中…').trigger('click')
  expect(mocks.synthesize).toHaveBeenCalledOnce()
  if (action === 'clear') await button(wrapper, '清空').trigger('click')
  else wrapper.unmount()
  finish({ ok: true, filePath: '/late.mp3', bytes: 456 })
  await flushPromises()
  expect(wrapper.find('audio').exists()).toBe(false)
  expect(mocks.toast).not.toHaveBeenCalled()
})
