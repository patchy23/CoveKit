/**
 * frpc 可执行文件：检测、上游版本列表、一键下载（进度走 `frp://download` 事件）
 * 下载失败不清空已配置路径（任务书 §10 风险对策）。
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ipc } from '../ipc'
import type { FrpBinaryInfo, FrpDownloadPayload, FrpReleaseInfo } from '../contracts'
import { estimateSpeed, pushSample, type SpeedSample } from './downloadSpeed'

/** frpc 二进制相关状态与操作 */
export function useFrpBinary() {
  /** 最近一次探测结果（null = 尚未探测） */
  const info = ref<FrpBinaryInfo | null>(null)
  /** 探测中 */
  const detecting = ref(false)
  /** 上游版本列表 */
  const versions = ref<FrpReleaseInfo[]>([])
  /** 版本列表加载中 */
  const loadingVersions = ref(false)
  /** 版本列表加载失败原因 */
  const versionsError = ref('')
  /** 下载中 */
  const downloading = ref(false)
  /** 下载进度（事件驱动） */
  const progress = ref<FrpDownloadPayload | null>(null)
  /** 下载速率（字节/秒；样本不足时为 null，表示暂无可显示值） */
  const speed = ref<number | null>(null)
  /** 下载失败原因 */
  const downloadError = ref('')
  /**
   * 下载成功的附带提示（如「未强校验」）
   *
   * 后端在「装好了但没能与上游 checksums 比对」时用 `error` 字段带出原因；
   * 这里必须接住并展示，否则用户无从知道这次安装有没有被强校验过。
   */
  const downloadWarning = ref('')

  let unlisten: UnlistenFn | null = null
  /** 速率采样窗口（每次下载重新累积） */
  let samples: SpeedSample[] = []

  /** 探测 frpc（给定路径优先，其次设置项 / PATH / 常见位置） */
  async function detect(path?: string): Promise<FrpBinaryInfo | null> {
    detecting.value = true
    try {
      info.value = await ipc.binaryDetect(path)
      return info.value
    } catch (reason) {
      info.value = {
        ok: false,
        error: reason instanceof Error ? reason.message : String(reason),
      }
      return info.value
    } finally {
      detecting.value = false
    }
  }

  /** 拉取上游可用版本（默认最近 10 个） */
  async function loadVersions(limit = 10): Promise<void> {
    loadingVersions.value = true
    versionsError.value = ''
    try {
      versions.value = await ipc.binaryVersions(limit)
    } catch (reason) {
      versions.value = []
      versionsError.value = reason instanceof Error ? reason.message : String(reason)
    } finally {
      loadingVersions.value = false
    }
  }

  /** 下载并安装指定版本（成功返回新的可执行文件信息） */
  async function download(version: string): Promise<FrpBinaryInfo | null> {
    downloading.value = true
    downloadError.value = ''
    downloadWarning.value = ''
    progress.value = null
    speed.value = null
    samples = []
    try {
      const result = await ipc.binaryDownload(version)
      if (!result.ok) throw new Error(result.error ?? '下载失败')
      info.value = result
      downloadWarning.value = result.error ?? ''
      return result
    } catch (reason) {
      downloadError.value = reason instanceof Error ? reason.message : String(reason)
      return null
    } finally {
      downloading.value = false
    }
  }

  onMounted(async () => {
    unlisten = await listen<FrpDownloadPayload>('frp://download', (event) => {
      const payload = event.payload
      progress.value = payload
      // 只在下载阶段累积采样：校验/解压阶段字节数不再增长，算了只会显示 0 B/s
      if (payload.phase !== 'download') return
      const now = Date.now()
      samples = pushSample(samples, { at: now, received: payload.received ?? 0 })
      speed.value = estimateSpeed(samples, now)
    })
  })
  onBeforeUnmount(() => {
    unlisten?.()
    unlisten = null
  })

  return {
    info,
    detecting,
    versions,
    loadingVersions,
    versionsError,
    downloading,
    progress,
    speed,
    downloadError,
    downloadWarning,
    detect,
    loadVersions,
    download,
  }
}
