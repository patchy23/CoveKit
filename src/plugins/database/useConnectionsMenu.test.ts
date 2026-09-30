import { ref } from 'vue'
import { expect, it, vi } from 'vitest'
import type { UiTreeItem } from '@/core/ui'
import type { DbConnectionInfo } from './contracts'
import type { useDatabase } from './useDatabase'
import { useConnectionsMenu } from './useConnectionsMenu'

vi.mock('@/core/feedback/useCopy', () => ({ useCopy: () => ({ copyText: vi.fn() }) }))

it('Redis 连接、逻辑库和键分组菜单共用滚动自动加载开关', () => {
  const connection: DbConnectionInfo = {
    id: 'redis-a',
    label: '测试 Redis',
    dbType: 'redis',
    env: 'test',
    status: 'online',
    version: '',
    latencyMs: 1,
    readonly: false,
    host: '127.0.0.1',
    database: 'db0',
    connectedAt: 0,
    port: 6379,
    username: '',
    ssl: false,
    connectTimeoutMs: 8000,
  }
  const redisAutoLoadEnabled = ref(false)
  const db = {
    connections: ref([connection]),
    redisAutoLoadEnabled,
    scopeContext: vi.fn(() => ({ database: 'db1', schema: '' })),
    openSqlEditorWithSql: vi.fn(),
    refreshTreeNode: vi.fn(),
    connect: vi.fn(),
    disconnect: vi.fn(),
    openSqlEditor: vi.fn(),
    removeConnection: vi.fn(),
    showSystemSchemas: ref({}),
    toggleSystemSchemas: vi.fn(),
  } as unknown as ReturnType<typeof useDatabase>
  const menu = useConnectionsMenu(db, vi.fn())
  const event = {
    preventDefault: vi.fn(),
    clientX: 0,
    clientY: 0,
  } as unknown as MouseEvent
  const nodes = [
    { connection },
    {
      item: {
        id: 'redis-a::redis:db1',
        label: 'db1',
        depth: 1,
        kind: 'database',
      } as UiTreeItem,
    },
    {
      item: {
        id: 'redis-a::redis:db1::keys',
        label: '键',
        depth: 2,
        kind: 'group-keys',
      } as UiTreeItem,
    },
  ]

  for (const node of nodes) {
    menu.openMenu(event, node)
    let option = menu.menuItems.value.find((item) => item.label === '启用滚动自动加载')
    expect(option).toBeDefined()
    option?.onClick?.()
    expect(redisAutoLoadEnabled.value).toBe(true)
    option = menu.menuItems.value.find((item) => item.label === '关闭滚动自动加载')
    expect(option).toBeDefined()
    option?.onClick?.()
    expect(redisAutoLoadEnabled.value).toBe(false)
  }
})
