/** 自动 JSON 预览保留原文回退；只缓存当前正文，隐藏后重展可复用已完成结果。 */
import { onScopeDispose, ref, watch } from 'vue'
import { createJsonFormatter } from './asyncJson'

export function useJsonPreview(source: () => string, enabled: () => boolean = () => true) {
  const formatter = createJsonFormatter()
  const content = ref(''),
    error = ref(''),
    pending = ref(false)
  let version = 0
  let cached: string | undefined
  watch(
    [source, enabled],
    async ([text, active]) => {
      const request = ++version
      formatter.cancel()
      pending.value = false
      error.value = ''
      if (text !== cached) {
        cached = undefined
        content.value = text
      }
      if (!active || text === cached) return
      pending.value = true
      try {
        const work = formatter.run(text)
        const result = work instanceof Promise ? await work : work
        if (request !== version || source() !== text || !enabled()) return
        content.value = result.ok ? result.output : text
        cached = text
      } catch (reason) {
        if (request === version) error.value = `格式化失败：${String(reason)}`
      } finally {
        if (request === version) pending.value = false
      }
    },
    { immediate: true }
  )
  onScopeDispose(() => {
    version++
    formatter.destroy()
  })
  return { content, error, pending }
}
