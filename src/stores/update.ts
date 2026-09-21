/**
 * 应用更新状态（可靠性 T11-2）
 *
 * 为什么从设置组件里搬出来：更新检查/下载原来存在 `UpdateSettingsCard` 的局部 ref，
 * 切走设置页就整个丢掉——下载到一半离开，回来只剩「检查更新」按钮，且句柄没人释放。
 *
 * 阶段划分与取消口径（这里是策略的唯一定义处）：
 * - `checking` / `latest` / `available`：可重新检查；
 * - `downloading`：**可取消**（关闭更新句柄，已下载内容丢弃）；
 * - `ready`：下载完成待安装，**可取消**（放弃已下载内容）；
 * - `installing`：**不可取消**——安装阶段替换文件并重启，中断会留下半装状态；
 * - 更新通道无效（占位公钥等）时进 `unavailable`，按钮禁用并说明原因。
 *
 * 重复保护：每个阶段都对同一动作做重入拦截，连点不会起第二次检查/下载。
 */
import { error as logError, info as logInfo } from '@tauri-apps/plugin-log'
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { ipc } from '@/core/ipc/ipc'
import { recordError } from '@/core/diagnostics/errors'

/** 更新阶段 */
export type UpdatePhase =
  | 'unknown'
  | 'unsupported'
  | 'unavailable'
  | 'idle'
  | 'checking'
  | 'latest'
  | 'available'
  | 'downloading'
  | 'ready'
  | 'installing'
  | 'error'

export const useUpdateStore = defineStore('update', () => {
  const phase = ref<UpdatePhase>('unknown')
  /** 当前更新句柄（未检查到更新时为空） */
  let handle: Update | null = null
  const version = ref('')
  const downloaded = ref(0)
  const total = ref<number | undefined>()
  /** 面向用户的错误说明 */
  const errorMessage = ref('')
  /** 稳定错误码（诊断用，如 update.download_failed） */
  const errorCode = ref('')
  /** 更新通道不可用原因（占位公钥、缺地址等） */
  const unavailableReason = ref('')
  /** 是否运行在桌面环境（浏览器预览下更新能力不存在） */
  const supported = '__TAURI_INTERNALS__' in window

  /** 下载百分比（总量未知时返回 null，界面显示已下载字节） */
  const percent = computed(() => {
    if (!total.value) return null
    return Math.min(100, Math.round((downloaded.value / total.value) * 100))
  })

  /** 是否允许检查更新 */
  const canCheck = computed(
    () =>
      supported &&
      phase.value !== 'checking' &&
      phase.value !== 'downloading' &&
      phase.value !== 'installing'
  )

  /** 是否允许取消当前动作（下载与下载完成待安装可取消；安装不可取消） */
  const canCancel = computed(() => phase.value === 'downloading' || phase.value === 'ready')

  /** 安装阶段不可取消的原因（界面直接展示） */
  const installNotCancellableReason = '安装阶段会替换程序文件并重启，中断可能留下装了一半的程序'

  /** 判定更新通道可用性（进设置页时调一次，也可主动刷新） */
  async function ensureAvailability(): Promise<void> {
    if (!supported) {
      phase.value = 'unsupported'
      return
    }
    try {
      const status = await ipc.updateAvailability()
      unavailableReason.value = status.available ? '' : status.reason
      if (!status.available && phase.value !== 'downloading' && phase.value !== 'installing') {
        phase.value = 'unavailable'
      } else if (status.available && phase.value === 'unknown') {
        phase.value = 'idle'
      }
    } catch (reason) {
      unavailableReason.value = reason instanceof Error ? reason.message : String(reason)
      phase.value = 'unavailable'
    }
  }

  /** 失败收口：统一记录错误码、给用户看的说明，并进诊断收集 */
  function fail(code: string, reason: unknown): void {
    errorCode.value = code
    errorMessage.value = reason instanceof Error ? reason.message : String(reason)
    phase.value = 'error'
    recordError({ code, message: errorMessage.value, source: 'update' })
    if (supported) {
      void logError(`更新操作失败 code=${code}`).catch(() => {
        console.warn('[diagnostics] 日志发送失败')
      })
    }
  }

  /** 关闭并清掉更新句柄（取消或收尾时调用，避免句柄泄漏） */
  async function releaseHandle(): Promise<void> {
    const current = handle
    handle = null
    if (!current) return
    try {
      await current.close()
    } catch (reason) {
      // 释放失败不是操作失败：记进诊断即可，不阻塞用户
      recordError({
        code: 'update.release_failed',
        message: reason instanceof Error ? reason.message : String(reason),
        source: 'update',
      })
      if (supported) {
        void logError('更新句柄释放失败 code=update.release_failed').catch(() => {
          console.warn('[diagnostics] 日志发送失败')
        })
      }
    }
  }

  /** 检查更新（可重复调用，但进行中会被挡住） */
  async function checkNow(): Promise<void> {
    if (!canCheck.value) return
    phase.value = 'checking'
    if (supported) {
      void logInfo('更新阶段 phase=checking').catch(() => {
        console.warn('[diagnostics] 日志发送失败')
      })
    }
    errorMessage.value = ''
    errorCode.value = ''
    try {
      const found = await check()
      handle = found
      if (!found) {
        version.value = ''
        phase.value = 'latest'
        return
      }
      version.value = found.version
      phase.value = 'available'
      if (supported) {
        void logInfo('更新阶段 phase=available').catch(() => {
          console.warn('[diagnostics] 日志发送失败')
        })
      }
    } catch (reason) {
      await releaseHandle()
      fail('update.check_failed', reason)
    }
  }

  /** 下载更新（不安装）：进行中重复调用无效 */
  async function download(): Promise<void> {
    if (!handle || phase.value === 'downloading' || phase.value === 'installing') return
    if (phase.value !== 'available' && phase.value !== 'ready') return
    phase.value = 'downloading'
    if (supported) {
      void logInfo('更新阶段 phase=downloading').catch(() => {
        console.warn('[diagnostics] 日志发送失败')
      })
    }
    downloaded.value = 0
    total.value = undefined
    errorMessage.value = ''
    errorCode.value = ''
    const current = handle
    try {
      await current.download((event) => {
        // 取消后句柄已释放：忽略迟到的进度，避免界面跳回下载中
        if (handle !== current) return
        if (event.event === 'Started') total.value = event.data.contentLength
        if (event.event === 'Progress') downloaded.value += event.data.chunkLength
      })
      if (handle !== current) return
      phase.value = 'ready'
      if (supported) {
        void logInfo('更新阶段 phase=ready').catch(() => {
          console.warn('[diagnostics] 日志发送失败')
        })
      }
    } catch (reason) {
      if (handle !== current) return
      fail('update.download_failed', reason)
    }
  }

  /** 取消下载或放弃已下载内容 */
  async function cancel(): Promise<void> {
    if (!canCancel.value) return
    await releaseHandle()
    downloaded.value = 0
    total.value = undefined
    version.value = ''
    phase.value = 'idle'
    if (supported) {
      void logInfo('更新操作已取消').catch(() => {
        console.warn('[diagnostics] 日志发送失败')
      })
    }
  }

  /** 安装已下载的更新并重启（不可取消） */
  async function install(): Promise<void> {
    if (!handle || phase.value !== 'ready') return
    phase.value = 'installing'
    if (supported) {
      void logInfo('更新阶段 phase=installing').catch(() => {
        console.warn('[diagnostics] 日志发送失败')
      })
    }
    try {
      await handle.install()
      await relaunch()
    } catch (reason) {
      fail('update.install_failed', reason)
    }
  }

  return {
    phase,
    version,
    downloaded,
    total,
    percent,
    errorMessage,
    errorCode,
    unavailableReason,
    supported,
    canCheck,
    canCancel,
    installNotCancellableReason,
    ensureAvailability,
    checkNow,
    download,
    cancel,
    install,
  }
})
