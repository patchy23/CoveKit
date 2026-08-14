/**
 * database-v2 插件入口
 * 说明：与现有 plugins/database 并存的新一代数据库工作台 UI 设计稿
 * 当前为纯前端模拟（不连接真实数据库），仅用于对比与迭代视觉/交互
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'database-v2',
  name: '数据库 v2',
  category: 'dev',
  icon: 'db',
  description: '新一代数据库工作台（设计稿）：紧凑三栏布局，全图标按钮，可隐藏摘要面板。',
  keywords: [
    'database',
    'sql',
    'mysql',
    'postgresql',
    'sqlite',
    'oracle',
    'redis',
    '数据库',
    '查询',
    'v2',
  ],
  presentation: 'workspace',
  component: () => import('./DatabaseV2Workbench.vue'),
  tags: ['第二批', '模拟数据', '设计稿'],
})
