import { computed, ref } from 'vue'
import type { UiTreeItem } from '@/core/ui'
import type { ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import { useCopy } from '@/core/ui/useClipboard'
import type { DbConnectionInfo } from './contracts'
import type { useDatabase } from './useDatabase'
import { supportsVisualCreateTable, type V2DbType } from './useDatabaseMeta'

const CREATE_DB_TYPES = new Set(['mysql', 'polardb', 'postgresql', 'kingbase', 'vastbase'])

export function useConnectionsMenu(
  db: ReturnType<typeof useDatabase>,
  editConnection: (connection: DbConnectionInfo) => void
) {
  const { copyText } = useCopy()
  const menu = ref<{
    x: number
    y: number
    connection?: DbConnectionInfo
    item?: UiTreeItem
  } | null>(null)
  const deleteTarget = ref<DbConnectionInfo | null>(null)
  const createDbFor = ref<DbConnectionInfo | null>(null)
  const dropDbTarget = ref<{ connId: string; name: string } | null>(null)
  const tableAction = ref<{
    mode: 'rename' | 'truncate' | 'drop'
    connId: string
    scope: string
    name: string
    kind: string
  } | null>(null)
  const renameValue = ref('')

  const menuItems = computed<ContextMenuItem[]>(() => {
    const target = menu.value
    if (!target) return []
    if (target.connection) return connectionItems(target.connection)
    const item = target.item
    if (!item) return []
    const connId = item.id.split('::')[0]
    const scope = item.id.split('::')[1] ?? ''
    const connection = db.connections.value.find((entry) => entry.id === connId)
    if (!connection) return []
    if (item.kind === 'database' && connection.dbType === 'redis') {
      return [{ label: '刷新键列表', onClick: () => void db.refreshTreeNode(item) }]
    }
    if (item.kind === 'database' || item.kind === 'schema' || item.kind?.startsWith('group')) {
      const context = db.scopeContext(connection, scope)
      const items: ContextMenuItem[] = [
        {
          label: '打开SQL编辑器',
          onClick: () => db.openSqlEditorWithSql(connId, '', context.database, context.schema),
        },
        {
          label: '新建表',
          onClick: () =>
            supportsVisualCreateTable(connection.dbType as V2DbType)
              ? db.openCreateTableTab(connId, scope)
              : db.openCreateTableEditor(connId, scope),
        },
        { label: '刷新', onClick: () => void db.refreshTreeNode(item) },
      ]
      if (item.kind === 'database' && CREATE_DB_TYPES.has(connection.dbType)) {
        items.push({ label: '', separator: true })
        items.push({
          label: '删除数据库',
          danger: true,
          onClick: () => (dropDbTarget.value = { connId, name: scope }),
        })
      }
      return items
    }
    const leaf = db.parseLeafId(item.id)
    if (!leaf) return []
    if (leaf.kind === 'table' || leaf.kind === 'view' || leaf.kind === 'materialized_view') {
      const context = db.scopeContext(connection, leaf.scope)
      const items: ContextMenuItem[] = [
        { label: '查看数据', onClick: () => void db.selectResource(item.id) },
        {
          label: '查看结构',
          onClick: () => db.openStructureTab(connId, leaf.name, context.database, context.schema),
        },
        { label: '复制名称', onClick: () => void copyText(leaf.name) },
      ]
      if (leaf.kind === 'table') {
        items.push({ label: '', separator: true })
        for (const [mode, label, danger] of [
          ['rename', '重命名', false],
          ['truncate', '清空表', true],
          ['drop', '删除表', true],
        ] as const) {
          items.push({
            label,
            danger,
            onClick: () => {
              tableAction.value = {
                mode,
                connId,
                scope: leaf.scope,
                name: leaf.name,
                kind: leaf.kind,
              }
              if (mode === 'rename') renameValue.value = leaf.name
            },
          })
        }
      }
      return items
    }
    if (leaf.kind === 'key') {
      return [
        { label: '查看键', onClick: () => void db.selectResource(item.id) },
        { label: '复制键名', onClick: () => void copyText(leaf.name) },
      ]
    }
    return [{ label: '复制名称', onClick: () => void copyText(leaf.name) }]
  })

  function connectionItems(connection: DbConnectionInfo): ContextMenuItem[] {
    const online = connection.status === 'online'
    const items: ContextMenuItem[] = [
      {
        label: online ? '断开连接' : '连接',
        onClick: () =>
          online ? void db.disconnect(connection) : void db.connect(connection).catch(() => {}),
      },
      { label: '编辑连接', onClick: () => editConnection(connection) },
      { label: '打开SQL编辑器', onClick: () => db.openSqlEditor(connection.id) },
    ]
    if (online && CREATE_DB_TYPES.has(connection.dbType)) {
      items.push({ label: '新建数据库', onClick: () => (createDbFor.value = connection) })
    }
    items.push({
      label: '刷新元数据',
      onClick: () =>
        void db.refreshTreeNode({
          id: connection.id,
          kind: 'connection',
          label: connection.label,
          depth: 0,
          expandable: false,
          expanded: false,
        }),
    })
    if (online && !['sqlite', 'redis'].includes(connection.dbType)) {
      items.push({
        label: db.showSystemSchemas.value[connection.id] ? '隐藏系统库' : '显示系统库',
        onClick: () => db.toggleSystemSchemas(connection.id),
      })
    }
    items.push({ label: '', separator: true })
    items.push({
      label: '删除连接',
      danger: true,
      onClick: () => (deleteTarget.value = connection),
    })
    return items
  }

  function openMenu(
    event: MouseEvent,
    payload: { connection?: DbConnectionInfo; item?: UiTreeItem }
  ) {
    event.preventDefault()
    menu.value = {
      x: Math.min(event.clientX, window.innerWidth - 132),
      y: Math.min(event.clientY, window.innerHeight - 220),
      ...payload,
    }
  }
  async function confirmDropDb() {
    const target = dropDbTarget.value
    if (target && (await db.dropDatabase(target.connId, target.name))) dropDbTarget.value = null
  }
  async function confirmTableAction() {
    const action = tableAction.value
    if (!action || (action.mode === 'rename' && !renameValue.value.trim())) return
    const ok = await db.tableAdminAction(
      action.connId,
      action.scope,
      action.name,
      action.mode,
      action.mode === 'rename' ? renameValue.value.trim() : undefined,
      action.kind
    )
    if (ok) tableAction.value = null
  }
  function confirmDelete() {
    if (!deleteTarget.value) return
    void db.removeConnection(deleteTarget.value.id)
    deleteTarget.value = null
  }

  return {
    menu,
    menuItems,
    deleteTarget,
    createDbFor,
    dropDbTarget,
    tableAction,
    renameValue,
    openMenu,
    confirmDropDb,
    confirmTableAction,
    confirmDelete,
  }
}
