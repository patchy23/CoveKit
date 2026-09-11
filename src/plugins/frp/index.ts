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
      key: 'frpcPath',
      type: 'text',
      label: 'frpc 可执行文件路径（留空则自动探测 PATH 与常见位置）',
      default: '',
    },
    {
      key: 'profileDir',
      type: 'text',
      label: '配置文件目录（留空则用应用数据目录下的 frp/profiles）',
      default: '',
    },
    {
      key: 'downloadMirror',
      type: 'text',
      label: '下载镜像前缀（GitHub 不可达时填写，例如 https://ghfast.top/）',
      default: '',
    },
    {
      key: 'maxLogLines',
      type: 'number',
      label: '日志缓冲行数',
      default: 2000,
    },
  ],
  tags: ['网络'],
})
