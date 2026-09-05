/**
 * HTTP/WS 调试 · 工具注册
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'http-ws',
  name: '接口调试',
  category: 'net',
  icon: 'net',
  description: 'HTTP 请求构建与 WebSocket 长连接调试，响应高亮预览。',
  keywords: ['http', 'https', '请求', 'api', 'websocket', 'ws', '调试', 'postman'],
  component: () => import('./index.vue'),
  tags: ['网络', '热门'],
})
