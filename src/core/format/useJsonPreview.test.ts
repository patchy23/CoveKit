import { effectScope, nextTick, ref } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { useJsonPreview } from './useJsonPreview'

afterEach(() => vi.useRealTimers())

it('隐藏时换正文不会误用旧缓存，重新展开仍自动格式化', async () => {
  const scope = effectScope()
  const text = ref('{"a":1}'),
    active = ref(true)
  const preview = scope.run(() =>
    useJsonPreview(
      () => text.value,
      () => active.value
    )
  )!
  try {
    await nextTick()
    await nextTick()
    expect(preview.content.value).toBe('{\n  "a": 1\n}')
    active.value = false
    await nextTick()
    text.value = 'raw'
    await nextTick()
    text.value = '{"a":1}'
    await nextTick()
    active.value = true
    await nextTick()
    await nextTick()
    expect(preview.content.value).toBe('{\n  "a": 1\n}')
    text.value = '  raw\ntext  '
    await nextTick()
    await nextTick()
    expect(preview.content.value).toBe(text.value)
    expect(preview.error.value).toBe('')
  } finally {
    scope.stop()
  }
})

it('格式化缓存只在隐藏闲置后释放，当前查看保留，重展重新格式化', async () => {
  vi.useFakeTimers()
  const scope = effectScope()
  const active = ref(true)
  const raw = '{"中文":[1,2,3]}'
  const preview = scope.run(() =>
    useJsonPreview(
      () => raw,
      () => active.value
    )
  )!
  const formatted = preview.content.value
  expect(formatted).not.toBe(raw)
  await vi.advanceTimersByTimeAsync(20 * 60 * 1000)
  expect(preview.content.value).toBe(formatted)
  active.value = false
  await nextTick()
  await vi.advanceTimersByTimeAsync(9 * 60 * 1000)
  expect(preview.content.value).toBe(formatted)
  active.value = true
  await nextTick()
  expect(vi.getTimerCount()).toBe(0)
  active.value = false
  await nextTick()
  await vi.advanceTimersByTimeAsync(10 * 60 * 1000)
  expect(preview.content.value).toBe(raw)
  expect(vi.getTimerCount()).toBe(0)
  active.value = true
  await nextTick()
  expect(preview.content.value).toBe(formatted)
  active.value = false
  await nextTick()
  scope.stop()
  expect(vi.getTimerCount()).toBe(0)
  expect(preview.content.value).toBe('')
})
