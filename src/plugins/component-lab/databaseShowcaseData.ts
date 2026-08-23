import type { UiDataGridColumn, UiTreeItem, UiWorkbenchTab } from '@/core/ui'

export const connectionTabs = [
  { value: 'mysql-prod', label: 'MySQL · 生产库', status: 'success' as const, closable: true },
  { value: 'kingbase-dev', label: 'Kingbase · 开发库', status: 'success' as const, closable: true },
  { value: 'oracle-test', label: 'Oracle · 测试库', status: 'danger' as const, closable: true },
]
export const createDocuments = (): UiWorkbenchTab[] => [
  { id: 'query-1', label: '查询 1', kind: 'query', dirty: true },
  { id: 'users-data', label: 'users · 数据', kind: 'data' },
  { id: 'users-structure', label: 'users · 结构', kind: 'structure' },
]
export const createTreeItems = (): UiTreeItem[] => [
  {
    id: 'db-patchybox',
    label: 'patchybox',
    depth: 0,
    kind: 'database',
    expandable: true,
    expanded: true,
  },
  {
    id: 'schema-public',
    label: 'public',
    depth: 1,
    kind: 'schema',
    expandable: true,
    expanded: true,
  },
  {
    id: 'tables',
    label: '表',
    depth: 2,
    kind: 'group',
    badge: 42,
    expandable: true,
    expanded: true,
  },
  { id: 'table-audit', label: 'audit_logs', depth: 3, kind: 'table' },
  { id: 'table-orders', label: 'orders', depth: 3, kind: 'table' },
  { id: 'table-users', label: 'users', depth: 3, kind: 'table' },
  { id: 'views', label: '视图', depth: 2, kind: 'group', badge: 6, expandable: true },
  { id: 'functions', label: '函数', depth: 2, kind: 'group', badge: 12, expandable: true },
  { id: 'system', label: '系统对象', depth: 1, kind: 'schema', expandable: true, muted: true },
]
export const columns: UiDataGridColumn[] = [
  { key: 'id', label: 'id', width: 64, align: 'right', content: 'numeric' },
  { key: 'username', label: 'username', width: 112, content: 'technical' },
  { key: 'display_name', label: 'display_name', width: 126 },
  { key: 'email', label: 'email', width: 208, content: 'technical' },
  { key: 'status', label: 'status', width: 82 },
  { key: 'created_at', label: 'created_at', width: 160, content: 'technical' },
]
export const rows = Array.from({ length: 48 }, (_, index) => ({
  id: String(index + 1),
  username: `user_${String(index + 1).padStart(3, '0')}`,
  display_name: ['林晓', '陈嘉禾', '王明远', '周宁'][index % 4],
  email: `user${index + 1}@patchybox.dev`,
  status: index % 7 === 0 ? 'locked' : 'active',
  created_at: `2026-08-${String((index % 12) + 1).padStart(2, '0')} 10:${String(index).padStart(2, '0')}:26`,
}))
export const databaseOptions = ['patchybox', 'information_schema'].map((value) => ({
  value,
  label: value,
}))
export const schemaOptions = ['public', 'audit'].map((value) => ({ value, label: value }))
export const limitOptions = ['100', '500', '1000', '5000'].map((value) => ({
  value,
  label: `${value} 行`,
}))
export const resultTabs = [
  { value: 'data', label: '结果 1', badge: 48 },
  { value: 'message', label: '消息' },
  { value: 'plan', label: '执行计划' },
]
