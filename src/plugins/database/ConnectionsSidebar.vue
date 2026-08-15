<script setup lang="ts">
/**
 * 左侧连接栏：搜索 + 新建连接 + 对象树（懒加载元数据）
 * 交互约定：单击仅选中；双击可展开节点折叠/展开，双击叶子打开（表→结构页签，键→键详情）；
 * 右键按层级出菜单（连接/库/分组/对象）；错误提示走状态点 tooltip，名字始终完整。
 */
import { computed, ref } from 'vue'
import {
  UiButton,
  UiIcon,
  UiIconButton,
  UiInput,
  UiModal,
  UiSearchInput,
  UiSpinner,
  UiTree,
  type UiTreeItem,
} from '@/core/ui'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { useCopy } from '@/core/ui/useClipboard'
import DbObjectIcon from './DbObjectIcon.vue'
import { useSplitPane } from './useSplitPane'
import { DB_TYPE_META } from './useDatabaseMeta'
import type { DbConnectionInfo } from './contracts'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const emit = defineEmits<{
  newConnection: []
  editConnection: [connection: DbConnectionInfo]
}>()

const { db } = props
const { copyText } = useCopy()

/** 右键菜单目标：连接节点带 connection；其余层级带树节点 item */
const menu = ref<{ x: number; y: number; connection?: DbConnectionInfo; item?: UiTreeItem } | null>(
  null
)
const deleteTarget = ref<DbConnectionInfo | null>(null)
/** 新建数据库弹窗状态 */
const createDbFor = ref<DbConnectionInfo | null>(null)
const createDbName = ref('')

/** 侧栏宽度（可拖拽，默认 200，范围 168–340；分隔条在面板右侧，正向） */
const { size: sidebarWidth, onPointerDown: onDragStart } = useSplitPane({
  initial: 200,
  min: 168,
  max: 340,
})

/** 支持 CREATE DATABASE 的类型（oracle/dameng 无建库语义，sqlite/redis 不适用） */
const CREATE_DB_TYPES = new Set(['mysql', 'polardb', 'postgresql', 'kingbase', 'vastbase'])

const menuItems = computed<ContextMenuItem[]>(() => {
  const target = menu.value
  if (!target) return []
  // ── 连接节点菜单 ──
  if (target.connection) {
    const connection = target.connection
    const online = connection.status === 'online'
    const items: ContextMenuItem[] = [
      {
        label: online ? '断开连接' : '连接',
        onClick: () => {
          if (online) void db.disconnect(connection)
          else void db.connect(connection).catch(() => {})
        },
      },
      { label: '编辑连接', onClick: () => emit('editConnection', connection) },
      { label: '打开SQL编辑器', onClick: () => db.openSqlEditor(connection.id) },
    ]
    if (online && CREATE_DB_TYPES.has(connection.dbType)) {
      items.push({
        label: '新建数据库',
        onClick: () => {
          createDbFor.value = connection
          createDbName.value = ''
        },
      })
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
    // 系统库显隐（有系统清单的类型才有意义）
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
  // ── 库/schema/分组/对象节点菜单 ──
  const item = target.item
  if (!item) return []
  const connId = item.id.split('::')[0]
  const scope = item.id.split('::')[1] ?? ''
  const conn = db.connections.value.find((c) => c.id === connId)
  if (!conn) return []
  // Redis 数据库节点：只提供刷新键
  if (item.kind === 'database' && conn.dbType === 'redis') {
    return [{ label: '刷新键列表', onClick: () => void db.refreshTreeNode(item) }]
  }
  if (item.kind === 'database' || item.kind === 'schema' || item.kind?.startsWith('group')) {
    return [
      {
        label: '打开SQL编辑器',
        onClick: () => {
          const { database, schema } = db.scopeContext(conn, scope)
          db.openSqlEditorWithSql(connId, '', database, schema)
        },
      },
      { label: '新建表', onClick: () => db.openCreateTableEditor(connId, scope) },
      { label: '刷新', onClick: () => void db.refreshTreeNode(item) },
    ]
  }
  const leaf = db.parseLeafId(item.id)
  if (!leaf) return []
  if (leaf.kind === 'table' || leaf.kind === 'view' || leaf.kind === 'materialized_view') {
    const { database, schema } = db.scopeContext(conn, leaf.scope)
    return [
      { label: '查看数据', onClick: () => void db.selectResource(item.id) },
      {
        label: '查看结构',
        onClick: () => db.openStructureTab(connId, leaf.name, database, schema),
      },
      { label: '复制名称', onClick: () => void copyText(leaf.name) },
    ]
  }
  if (leaf.kind === 'key') {
    return [
      { label: '查看键', onClick: () => void db.selectResource(item.id) },
      { label: '复制键名', onClick: () => void copyText(leaf.name) },
    ]
  }
  // 其余叶子（函数/序列等）：仅复制名称
  return [{ label: '复制名称', onClick: () => void copyText(leaf.name) }]
})

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

async function confirmCreateDb() {
  const conn = createDbFor.value
  if (!conn) return
  const ok = await db.createDatabase(conn.id, createDbName.value)
  if (ok) createDbFor.value = null
}

function confirmDelete() {
  if (deleteTarget.value) {
    void db.removeConnection(deleteTarget.value.id)
    deleteTarget.value = null
  }
}

function onTreeSelect(id: string) {
  // 单击仅选中（并同步活动连接）；打开页签由双击（open）与右键菜单承担
  const connId = id.split('::')[0]
  if (db.connections.value.some((c) => c.id === connId)) db.activeConnectionId.value = connId
}

/** 双击叶子节点：表/视图 → 结构页签；Redis 键 → 键详情页签 */
function onTreeOpen(item: UiTreeItem) {
  const leaf = db.parseLeafId(item.id)
  if (!leaf) return
  const conn = db.connections.value.find((c) => c.id === leaf.connId)
  if (!conn) return
  if (leaf.kind === 'table' || leaf.kind === 'view' || leaf.kind === 'materialized_view') {
    const { database, schema } = db.scopeContext(conn, leaf.scope)
    db.openStructureTab(leaf.connId, leaf.name, database, schema)
    return
  }
  if (leaf.kind === 'key') void db.selectResource(item.id)
}

function onTreeContext(mouse: MouseEvent, item: UiTreeItem) {
  if (item.depth === 0) {
    const connection = db.connections.value.find((c) => c.id === item.id)
    if (connection) openMenu(mouse, { connection })
    return
  }
  openMenu(mouse, { item })
}
</script>

<template>
  <aside
    class="relative flex shrink-0 flex-col border-r border-border dark:border-border-dark"
    :style="{ width: `${sidebarWidth}px` }"
  >
    <div class="shrink-0 space-y-[8px] px-[10px] py-[10px]">
      <UiSearchInput v-model="db.keyword.value" size="sm" placeholder="搜索连接…" />
      <UiButton variant="secondary" size="sm" block @click="emit('newConnection')">
        + 新建连接
      </UiButton>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-[4px] pb-[8px]">
      <UiTree
        v-model="db.selectedResource.value"
        :items="db.visibleTreeItems.value"
        :row-height="24"
        @update:model-value="onTreeSelect"
        @toggle="db.toggleTree"
        @open="onTreeOpen"
        @context="onTreeContext"
      >
        <template #icon="{ item }">
          <img
            v-if="item.depth === 0"
            :src="
              DB_TYPE_META[
                db.connections.value.find((c) => c.id === item.id)
                  ?.dbType as keyof typeof DB_TYPE_META
              ]?.icon ?? ''
            "
            :alt="item.id"
            class="h-[16px] w-[16px] shrink-0 object-contain"
          />
          <span
            v-else
            class="grid w-[16px] shrink-0 place-items-center text-text-muted dark:text-text-muted-dark"
          >
            <DbObjectIcon :kind="item.kind" />
          </span>
        </template>
        <template #suffix="{ item }">
          <template v-if="item.depth === 0">
            <!-- 连接中：spinner + 取消按钮 -->
            <template v-if="db.connecting.value[item.id]">
              <UiSpinner size="xs" />
              <UiIconButton
                label="取消连接"
                size="xs"
                class="text-text-muted dark:text-text-muted-dark"
                @click.stop="db.cancelConnect(item.id)"
              >
                <UiIcon name="x" :size="10" :stroke-width="2.5" />
              </UiIconButton>
            </template>
            <!-- 状态点：在线绿 / 离线灰红，错误 hover 提示 -->
            <span
              v-else
              class="h-[6px] w-[6px] shrink-0 rounded-full"
              :class="
                db.connectError.value[item.id]
                  ? 'bg-danger-strong dark:bg-danger-dark'
                  : db.connections.value.find((c) => c.id === item.id)?.status === 'online'
                    ? 'bg-success-strong dark:bg-success-dark'
                    : 'bg-text-muted/40 dark:bg-text-muted-dark/40'
              "
              :title="
                db.connectError.value[item.id] ||
                (db.connections.value.find((c) => c.id === item.id)?.status === 'online'
                  ? '已连接'
                  : '未连接')
              "
            />
          </template>
        </template>
      </UiTree>

      <p
        v-if="!db.visibleTreeItems.value.length"
        class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        {{ db.keyword.value ? '无匹配对象' : '暂无连接，点击上方新建' }}
      </p>
    </div>

    <!-- 宽度拖拽手柄 -->
    <div
      class="absolute -right-[3px] top-0 z-10 h-full w-[6px] cursor-col-resize hover:bg-tertiary/30"
      @mousedown="onDragStart"
    />

    <ConfirmDialog
      :open="deleteTarget !== null"
      title="删除连接"
      :message="`确定删除「${deleteTarget?.label ?? ''}」吗？该连接下的所有查询页签将被关闭。`"
      confirm-label="删除"
      danger
      @close="deleteTarget = null"
      @confirm="confirmDelete"
    />

    <!-- 新建数据库 -->
    <UiModal :open="createDbFor !== null" title="新建数据库" size="sm" @close="createDbFor = null">
      <div class="space-y-[8px]">
        <p class="text-body-sm text-secondary dark:text-secondary-dark">
          在「{{ createDbFor?.label ?? '' }}」上执行 CREATE DATABASE，创建成功后自动刷新库列表。
        </p>
        <UiInput
          v-model="createDbName"
          size="sm"
          placeholder="数据库名（字母开头，可含数字/下划线/$）"
          @keydown.enter="confirmCreateDb"
        />
      </div>
      <template #footer>
        <UiButton size="sm" variant="ghost" @click="createDbFor = null">取消</UiButton>
        <UiButton
          size="sm"
          variant="primary"
          :disabled="!createDbName.trim()"
          @click="confirmCreateDb"
        >
          创建
        </UiButton>
      </template>
    </UiModal>

    <ContextMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      size="sm"
      @close="menu = null"
    />
  </aside>
</template>
