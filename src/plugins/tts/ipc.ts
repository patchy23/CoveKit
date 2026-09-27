/**
 * 文字转语音插件 · IPC 封装（本插件命令，独立于框架）
 */
import { invokeCommand } from '@/core/ipc/ipc'
import { Channel } from '@tauri-apps/api/core'
import type { Payloads, Results, TtsProgress } from './contracts'

export const ipc = {
  /** 获取语音列表 */
  ttsVoices: (): Promise<Results['tts_voices']> => invokeCommand('tts_voices', {}),
  /** 登记请求后再提交正文，保证取消信号有明确归属。 */
  ttsPrepare: (): Promise<Results['tts_prepare']> => invokeCommand('tts_prepare', {}),
  ttsCancel: (jobId: string): Promise<Results['tts_cancel']> =>
    invokeCommand('tts_cancel', { jobId }),
  /** 只释放已返回但从未交付给播放器或下载链接的过期结果。 */
  ttsDiscard: (jobId: string): Promise<Results['tts_discard']> =>
    invokeCommand('tts_discard', { jobId }),
  /** 合成语音（文本 + 语音 + 语速/音调 → mp3 文件路径） */
  ttsSynthesize: (
    p: Omit<Payloads['tts_synthesize'], 'onProgress'>,
    receive?: (progress: TtsProgress) => void
  ): Promise<Results['tts_synthesize']> => {
    if (!receive) return invokeCommand('tts_synthesize', p)
    const onProgress = new Channel<TtsProgress>()
    onProgress.onmessage = receive
    return invokeCommand<Payloads['tts_synthesize'], Results['tts_synthesize']>('tts_synthesize', {
      ...p,
      onProgress,
    }).finally(() => {
      onProgress.onmessage = () => {}
    })
  },
}
