/**
 * database 插件入口
 * 多连接数据库工作台：对象树导航、SQL 查询、结果浏览与表结构查看。
 * 驱动：mysql/polardb（mysql_async）、postgresql（tokio-postgres）、
 * sqlite（rusqlite）、redis（redis crate）、oracle/vastbase/kingbase（agent 侧车）。
 */
import { registerTool } from '@/core/registry/toolRegistry'

registerTool({
  id: 'database',
  name: '数据库',
  category: 'dev',
  icon: 'db',
  description: '多连接数据库工作台：对象树导航、SQL 查询、结果浏览与表结构查看。',
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
  ],
  presentation: 'workspace',
  component: () => import('./DatabaseWorkbench.vue'),
  tags: ['第二批'],
})
