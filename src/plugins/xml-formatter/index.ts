/**
 * XML 格式化 · 工具注册
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'xml-formatter',
  name: 'XML 格式化',
  category: 'dev',
  icon: 'xml',
  description: 'XML 格式化、压缩与语法校验，支持声明、注释与自闭合标签。',
  keywords: ['xml', '格式化', 'formatter', '压缩', '校验', 'pretty'],
  presentation: 'workspace',
  component: () => import('./index.vue'),
  tags: ['开发'],
})
