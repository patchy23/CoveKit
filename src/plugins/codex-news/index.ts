/** Codex 重置消息工具注册；页面与网络请求按需加载。 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'codex-news',
  name: 'Codex 重置消息',
  category: 'dev',
  icon: 'clipboard',
  description: '查看 Tibo 的 Codex 额度重置消息、历史动态和原文来源。',
  keywords: ['codex', 'tibo', '重置', '额度', '消息'],
  component: () => import('./index.vue'),
})
