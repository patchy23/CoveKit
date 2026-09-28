/** 存储维护前统一协商工具关闭；有未保存内容、清理失败或超时均不继续切换。 */
import { nextTick, ref } from 'vue'
import { collectAllToolBlockers, disposeAllTools } from '@/core/lifecycle/toolContext'
import { requestBackendClose } from '@/core/lifecycle/closeBridge'
import { useUiStore } from '@/stores/ui'

type Translate = (key: string) => string

/** 只允许经过非强制关闭协商的迁移或恢复操作，完成后重建当前 WebView。 */
export function useStorageMaintenance(t: Translate, reload = () => window.location.reload()) {
  const ui = useUiStore()
  const busy = ref(false)
  const error = ref('')

  async function run(operation: () => Promise<unknown>) {
    if (busy.value) return false
    busy.value = true
    error.value = ''
    try {
      const blockers = await collectAllToolBlockers('restart')
      const decision = await requestBackendClose('restart', null, blockers, false)
      if (
        blockers.length ||
        !decision.proceed ||
        decision.blockers.length ||
        decision.failures.length
      ) {
        const details = [
          ...new Set([
            ...blockers.map(({ owner, message }) => `${owner}: ${message}`),
            ...decision.blockers,
            ...decision.failures,
          ]),
        ]
        throw new Error(`${t('settings.storageBlocked')}\n${details.join('\n')}`)
      }
      const cleanup = await disposeAllTools('restart')
      if (cleanup.timedOut || cleanup.failures.length) {
        throw new Error(
          `${t('settings.storageCleanupFailed')}\n${cleanup.failures.map(({ owner, message }) => `${owner}: ${message}`).join('\n')}`
        )
      }
      ui.clearDisposedTabs()
      await nextTick()
      await operation()
      reload()
      return true
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : String(reason)
      return false
    } finally {
      busy.value = false
    }
  }

  return { busy, error, run }
}
