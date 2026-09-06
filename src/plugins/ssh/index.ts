/**
 * SSH 工具 · 工具注册
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'ssh',
  name: 'SSH 远程管理',
  category: 'net',
  icon: 'net',
  description: 'SSH 远程运维一体化：终端、文件传输、监控、服务与 Docker 管理。',
  keywords: ['ssh', 'terminal', 'sftp', '远程', '服务器', 'linux', '终端'],
  component: () => import('./index.vue'),
  settingsSchema: [
    {
      key: 'idleDisconnectMinutes',
      type: 'select',
      label: '空闲自动断开',
      default: '10',
      options: [
        { value: '1', label: '1 分钟（调试用）' },
        { value: '10', label: '10 分钟' },
        { value: '30', label: '30 分钟' },
        { value: '60', label: '1 小时' },
        { value: '120', label: '2 小时' },
        { value: '0', label: '永不自动断开' },
      ],
    },
    {
      key: 'autoReconnect',
      type: 'toggle',
      label: '意外断线自动重连（指数退避，最多 5 次）',
      default: true,
    },
  ],
  tags: ['网络', '热门'],
})
