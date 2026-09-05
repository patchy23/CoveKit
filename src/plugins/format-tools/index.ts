/**
 * 格式转换 · 工具注册（JSON / XML / 时间戳 / Base64 四合一，子页签切换）
 * 合并自原 json-formatter / xml-formatter / ts-converter 三个独立工具（2026-08-16 用户决策）。
 */
import { registerTool } from '@/core/registry/toolRegistry'
import type { SettingsField } from '@/core/registry/types'

const settingsSchema: SettingsField[] = [
  {
    key: 'indent',
    type: 'select',
    label: 'JSON 缩进宽度',
    default: 2,
    options: [
      { label: '2 空格', value: '2' },
      { label: '4 空格', value: '4' },
      { label: 'Tab', value: 'tab' },
    ],
  },
]

registerTool({
  id: 'format-tools',
  name: '格式转换',
  category: 'dev',
  icon: 'json',
  description: 'JSON / XML 格式化校验、时间戳互转、Base64 编解码。',
  keywords: ['json', 'xml', 'base64', '时间戳', 'timestamp', '格式化', '编码', '解码'],
  component: () => import('./FormatTools.vue'),
  settingsSchema,
  tags: ['开发', '热门'],
})
