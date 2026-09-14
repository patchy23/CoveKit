/**
 * FRP 客户端工具 · 工具注册
 * 管理本机 frpc 配置文件：表单/源码双模式编辑、frpc verify 校验、一键启停与实时日志。
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'frp',
  name: 'FRP 客户端',
  category: 'net',
  icon: 'frp',
  description: '管理本机 frpc 配置文件：双模式编辑、配置校验、一键启停与实时日志。',
  keywords: ['frp', 'frpc', 'frps', '内网穿透', '穿透', '隧道', 'tunnel', '反向代理', 'toml'],
  component: () => import('./FrpWorkbench.vue'),
  settingsSchema: [
    {
      key: 'profileDir',
      type: 'text',
      label: '配置文件目录（留空则用应用数据目录下的 frp/profiles）',
      default: '',
    },
  ],
  tags: ['网络'],
})
