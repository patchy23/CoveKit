/**
 * vault 凭证管理 · 框架页入口（隐藏工具）
 * hidden：不进工具库网格/搜索/分类计数；通过设置弹窗「凭证管理 → 管理凭证」
 * 触发 ui.openTool('vault') 以 workspace 页签打开。
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'vault',
  name: '凭证管理',
  category: 'sys',
  icon: 'lock',
  description: '凭证库：加密存储密码/密钥/Token，插件只引用凭证 ID。',
  keywords: ['vault', 'credential', '凭证', '密码'],
  presentation: 'workspace',
  hidden: true,
  component: () => import('./CredentialManagerPage.vue'),
})
