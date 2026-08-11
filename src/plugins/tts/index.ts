/**
 * 文字转语音 · 工具注册（自注册：建目录 + 注册一行，框架零改动）
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'tts',
  name: '文字转语音',
  category: 'text',
  icon: 'tts',
  description: '输入文字生成中文语音（晓晓/云希等 11 种音色），支持语速与音调调节。',
  keywords: ['tts', '语音', '朗读', '合成', '配音', 'text-to-speech', 'audio'],
  presentation: 'workspace',
  component: () => import('./index.vue'),
})
