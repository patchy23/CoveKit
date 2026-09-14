/**
 * 公共组件实验室 · 仅开发构建注册的组件验收工具，不承载业务能力。
 *
 * 正式版不出现：`import.meta.env.DEV` 在构建时被静态替换为 false，注册块与 `index.vue`
 * 的动态导入一并被摇除；开发构建（`pnpm dev`）照常可用。
 */
import { registerTool } from '@/core/registry/toolRegistry'

if (import.meta.env.DEV) {
  registerTool({
    id: 'component-lab',
    name: '组件实验室',
    category: 'dev',
    icon: 'grid',
    description: '临时查看与交互测试 patchyBox 公共前端组件的状态和组合效果。',
    keywords: ['ui', '组件', '设计系统', 'component', 'design system', '测试'],
    component: () => import('./index.vue'),
    tags: ['开发'],
  })
}
