/** 文件占用查询 · Windows 系统工具入口。 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'file-lock',
  name: '文件占用查询',
  category: 'sys',
  icon: 'file-lock',
  description: '查找使用某个文件的进程、PID 和程序路径，帮助定位无法删除的文件。',
  keywords: ['文件', '占用', '进程', '删除', '锁定', 'PID', 'Windows', 'file', 'lock'],
  component: () => import('./index.vue'),
  tags: ['Windows', '系统'],
})
