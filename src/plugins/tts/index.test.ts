import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import Tts from './index.vue'
import type { TtsResult } from './contracts'

const mocks = vi.hoisted(() => ({
  synthesize: vi.fn(),
  voices: vi.fn(),
  toast: vi.fn(),
  prepare: vi.fn(),
  cancel: vi.fn(),
  discard: vi.fn(),
}))
vi.mock('./ipc', () => ({
  ipc: {
    ttsSynthesize: mocks.synthesize,
    ttsVoices: mocks.voices,
    ttsPrepare: mocks.prepare,
    ttsCancel: mocks.cancel,
    ttsDiscard: mocks.discard,
  },
}))
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
  UiTextarea: {
    props: ['modelValue'],
    emits: ['update:modelValue'],
    template:
      '<textarea :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />',
  },
}))
enableAutoUnmount(afterEach)
beforeEach(() => {
  vi.clearAllMocks()
  mocks.voices.mockResolvedValue([])
  mocks.prepare.mockResolvedValue('job')
  mocks.cancel.mockResolvedValue(undefined)
  mocks.discard.mockResolvedValue(undefined)
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
  expect(mocks.discard).not.toHaveBeenCalled()
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
  expect(mocks.cancel).toHaveBeenCalledWith('job')
  expect(mocks.discard).toHaveBeenCalledWith('job')
})

it.each(['clear', 'close'])('%s 早于请求登记完成时取消登记结果，不启动合成', async (action) => {
  let prepare!: (id: string) => void
  mocks.prepare.mockReturnValueOnce(
    new Promise<string>((resolve) => {
      prepare = resolve
    })
  )
  const wrapper = mount(Tts)
  await button(wrapper, '合成语音').trigger('click')
  if (action === 'clear') await button(wrapper, '清空').trigger('click')
  else wrapper.unmount()
  prepare('late-registration')
  await flushPromises()
  expect(mocks.synthesize).not.toHaveBeenCalled()
  expect(mocks.cancel).toHaveBeenCalledWith('late-registration')
})

it('合成 IPC 失败也回收登记并允许重试', async () => {
  mocks.synthesize.mockRejectedValueOnce(new Error('invoke failed'))
  const wrapper = mount(Tts)
  await button(wrapper, '合成语音').trigger('click')
  await flushPromises()
  expect(mocks.cancel).toHaveBeenCalledWith('job')
  expect(mocks.toast).toHaveBeenCalledWith(expect.stringContaining('invoke failed'))
  expect(button(wrapper, '合成语音').exists()).toBe(true)
})

it('过期结果清理失败可见，仍不恢复旧播放器', async () => {
  let finish!: (result: TtsResult) => void
  mocks.synthesize.mockReturnValueOnce(new Promise<TtsResult>((resolve) => (finish = resolve)))
  mocks.discard.mockRejectedValueOnce(new Error('文件占用'))
  const wrapper = mount(Tts)
  await button(wrapper, '合成语音').trigger('click')
  await flushPromises()
  await button(wrapper, '清空').trigger('click')
  finish({ ok: true, filePath: '/late.mp3', bytes: 1 })
  await flushPromises()
  expect(mocks.toast).toHaveBeenCalledWith(expect.stringContaining('文件占用'))
  expect(wrapper.find('audio').exists()).toBe(false)
})

it('清空后可以立即新建合成，旧请求结束不解除新请求的忙碌状态', async () => {
  let first!: (result: TtsResult) => void
  let second!: (result: TtsResult) => void
  mocks.prepare.mockResolvedValueOnce('first').mockResolvedValueOnce('second')
  mocks.synthesize
    .mockReturnValueOnce(
      new Promise<TtsResult>((resolve) => {
        first = resolve
      })
    )
    .mockReturnValueOnce(
      new Promise<TtsResult>((resolve) => {
        second = resolve
      })
    )
  const wrapper = mount(Tts)
  await button(wrapper, '合成语音').trigger('click')
  await flushPromises()
  await button(wrapper, '清空').trigger('click')
  await wrapper.get('textarea').setValue('新的合成')
  await button(wrapper, '合成语音').trigger('click')
  await flushPromises()
  expect(mocks.cancel).toHaveBeenCalledWith('first')
  expect(mocks.synthesize).toHaveBeenLastCalledWith(
    expect.objectContaining({ jobId: 'second', text: '新的合成' }),
    expect.any(Function)
  )
  first({ ok: true, bytes: 4, filePath: '/old.mp3' })
  await flushPromises()
  expect(mocks.discard).toHaveBeenCalledWith('first')
  expect(mocks.discard).not.toHaveBeenCalledWith('second')
  expect(button(wrapper, '合成中…').exists()).toBe(true)
  expect(mocks.toast).not.toHaveBeenCalled()
  second({ ok: false, bytes: 0, error: '第二次失败' })
  await flushPromises()
  expect(button(wrapper, '合成语音').exists()).toBe(true)
})

it('取消新的合成保留已经呈现的音频、下载源和输入正文', async () => {
  const wrapper = mount(Tts)
  await button(wrapper, '合成语音').trigger('click')
  await flushPromises()
  expect(wrapper.get('audio').attributes('src')).toBe('asset:/test.mp3')
  mocks.synthesize.mockReturnValueOnce(new Promise(() => {}))
  await button(wrapper, '合成语音').trigger('click')
  await flushPromises()
  const callback = mocks.synthesize.mock.lastCall![1]
  callback({ jobId: 'job', sequence: 1, phase: 'receiving', lastProgressAt: Date.now(), bytes: 12 })
  await button(wrapper, '取消合成').trigger('click')
  expect(mocks.cancel).toHaveBeenCalledWith('job')
  expect(wrapper.get('audio').attributes('src')).toBe('asset:/test.mp3')
  expect(wrapper.get('textarea').element.value).toContain('你好')
  expect(mocks.discard).not.toHaveBeenCalled()
})
