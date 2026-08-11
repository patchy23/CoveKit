/**
 * 文字转语音插件 · IPC 契约（本插件私有，独立于框架与其它插件）
 * 与 Rust 侧 src-tauri/src/plugins/tts.rs 的 serde 结构逐字段对应。
 */

/** 语音项 */
export interface TtsVoice {
  /** 语音标识（Edge TTS 名称，如 zh-CN-XiaoxiaoNeural） */
  name: string
  /** 展示名（中文描述） */
  label: string
  /** 语言代码 */
  lang: string
}

/** 合成结果 */
export interface TtsResult {
  /** 是否成功 */
  ok: boolean
  /** 生成的音频文件路径（convertFileSrc 后播放） */
  filePath?: string
  /** 音频字节数 */
  bytes: number
  /** 错误信息（失败时） */
  error?: string
}

/** 命令入参 */
export type Payloads = {
  tts_voices: Record<string, never>
  tts_synthesize: { text: string; voice: string; rate?: number; pitch?: number }
}

/** 命令返回 */
export type Results = {
  tts_voices: TtsVoice[]
  tts_synthesize: TtsResult
}
