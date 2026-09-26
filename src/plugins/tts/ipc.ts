/**
 * 文字转语音插件 · IPC 封装（本插件命令，独立于框架）
 */
import { invokeCommand } from '@/core/ipc/ipc'
import type { Payloads, Results } from './contracts'

export const ipc = {
  /** 获取语音列表 */
  ttsVoices: (): Promise<Results['tts_voices']> => invokeCommand('tts_voices', {}),
  /** 登记请求后再提交正文，保证取消信号有明确归属。 */
  ttsPrepare: (): Promise<Results['tts_prepare']> => invokeCommand('tts_prepare', {}),
  ttsCancel: (jobId: string): Promise<Results['tts_cancel']> =>
    invokeCommand('tts_cancel', { jobId }),
  /** 合成语音（文本 + 语音 + 语速/音调 → mp3 文件路径） */
  ttsSynthesize: (p: Payloads['tts_synthesize']): Promise<Results['tts_synthesize']> =>
    invokeCommand('tts_synthesize', p),
}
