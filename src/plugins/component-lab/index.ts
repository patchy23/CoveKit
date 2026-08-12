/** 公共组件实验室 · 临时独立验收工具，不承载业务能力。 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'component-lab',
  name: '组件实验室',
  category: 'dev',
  icon: 'grid',
  description: '临时查看与交互测试 patchyBox 公共前端组件的状态和组合效果。',
  keywords: ['ui', '组件', '设计系统', 'component', 'design system', '测试'],
  presentation: 'workspace',
  component: () => import('./index.vue'),
  tags: ['开发'],
})
