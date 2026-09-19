/** 端口占用查询 · 独立 Windows 系统工具入口。 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'port-viewer',
  name: '端口占用查询',
  category: 'sys',
  icon: 'port-viewer',
  description: '按端口、进程名或 PID 查找本机 TCP/UDP 占用，定位并关闭相关进程。',
  keywords: ['端口', '占用', '监听', '进程', 'PID', 'TCP', 'UDP', 'port', 'Windows'],
  component: () => import('./index.vue'),
  tags: ['Windows', '系统'],
})
