/**
 * hosts 修改 · 工具注册
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'hosts',
  name: 'Hosts 编辑',
  category: 'sys',
  icon: 'hosts',
  description: '安全编辑 hosts 文件：语法校验、保存前自动备份，无需常驻管理员。',
  keywords: ['hosts', '域名', 'host', '解析', '屏蔽', '本地', '系统文件'],
  component: () => import('./index.vue'),
  tags: ['系统'],
})
