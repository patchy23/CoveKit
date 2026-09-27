/** 自动 JSON 预览保留原文回退；隐藏后短期复用结果，长期闲置释放格式化副本。 */
import { onScopeDispose, ref, watch } from 'vue'
import { createJsonFormatter } from './asyncJson'

const IDLE_CACHE_MS = 10 * 60 * 1000

export function useJsonPreview(source: () => string, enabled: () => boolean = () => true) {
  const formatter = createJsonFormatter()
  const content = ref(''),
    error = ref(''),
    pending = ref(false)
  let version = 0
  let cached: string | undefined
  let expiry: ReturnType<typeof setTimeout> | undefined
  function cancelExpiry() {
    if (expiry !== undefined) clearTimeout(expiry)
    expiry = undefined
  }
  watch(
    [source, enabled],
    async ([text, active]) => {
      const request = ++version
      cancelExpiry()
      formatter.cancel()
      pending.value = false
      error.value = ''
      if (text !== cached) {
        cached = undefined
        content.value = text
      }
      if (!active) {
        if (cached !== undefined && content.value !== text) {
          expiry = setTimeout(() => {
            expiry = undefined
            if (enabled()) return
            cached = undefined
            content.value = source()
          }, IDLE_CACHE_MS)
        }
        return
      }
      if (text === cached) return
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
    cancelExpiry()
    cached = undefined
    content.value = ''
    formatter.destroy()
  })
  return { content, error, pending }
}
