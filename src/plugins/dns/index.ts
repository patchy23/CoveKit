/**
 * DNS 工具 · 工具注册
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'dns',
  name: 'DNS 解析',
  category: 'net',
  icon: 'dns',
  description: '多类型、多服务器 DNS 查询对比，阿里云 / 腾讯云 / Cloudflare 解析管理。',
  keywords: [
    'dns',
    '域名',
    '解析',
    'dig',
    'nslookup',
    '查询',
    'A记录',
    'CNAME',
    'MX',
    'TXT',
    '阿里云',
    'dnspod',
    '腾讯云',
    'cloudflare',
    '云解析',
  ],
  component: () => import('./index.vue'),
  tags: ['网络'],
})
