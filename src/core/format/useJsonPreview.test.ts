import { effectScope, nextTick, ref } from 'vue'
import { expect, it } from 'vitest'
import { useJsonPreview } from './useJsonPreview'

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
