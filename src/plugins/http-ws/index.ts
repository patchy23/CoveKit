/**
 * HTTP/WS 调试 · 工具注册
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'http-ws',
  name: '接口调试',
  category: 'net',
  icon: 'net',
  description: 'HTTP、SSE 与 WebSocket 多页签调试，保存和分组管理常用接口。',
  keywords: [
    'http',
    'https',
    'sse',
    'event-stream',
    '请求',
    'api',
    'websocket',
    'ws',
    '调试',
    'postman',
  ],
  component: () => import('./index.vue'),
  tags: ['网络', '热门'],
})
