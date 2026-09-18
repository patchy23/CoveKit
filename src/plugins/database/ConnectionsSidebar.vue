<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { UiTooltip } from '@/core/ui'
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
import ContextMenu from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import CreateDatabaseDialog from './CreateDatabaseDialog.vue'
import DbObjectIcon from './DbObjectIcon.vue'
import { useSplitPane } from '@/core/ui/useSplitPane'
import { DB_TYPE_META } from './useDatabaseMeta'
import type { DbConnectionInfo } from './contracts'
import type { useDatabase } from './useDatabase'
import { useConnectionsMenu } from './useConnectionsMenu'

const props = defineProps<{ db: ReturnType<typeof useDatabase> }>()
const emit = defineEmits<{ newConnection: []; editConnection: [connection: DbConnectionInfo] }>()
const { db } = props
const { size: sidebarWidth, onPointerDown: onDragStart } = useSplitPane({
  initial: 200,
  min: 168,
  max: 340,
})
const {
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
} = useConnectionsMenu(db, (connection) => emit('editConnection', connection))

function onTreeSelect(id: string) {
  const connectionId = id.split('::')[0]
  if (db.connections.value.some((connection) => connection.id === connectionId)) {
    db.activeConnectionId.value = connectionId
  }
}
function onTreeOpen(item: UiTreeItem) {
  const connection = db.connections.value.find((entry) => entry.id === item.id)
  if (connection) {
    if (connection.status !== 'online' && !db.connecting.value[connection.id]) {
      void db.connect(connection).catch(() => {})
    }
    return
  }
  const leaf = db.parseLeafId(item.id)
  if (!leaf) return
  const parent = db.connections.value.find((entry) => entry.id === leaf.connId)
  if (!parent) return
  if (leaf.kind === 'table' || leaf.kind === 'view' || leaf.kind === 'materialized_view') {
    const context = db.scopeContext(parent, leaf.scope)
    db.openStructureTab(leaf.connId, leaf.name, context.database, context.schema)
  } else if (leaf.kind === 'key') {
    void db.selectResource(item.id)
  }
}
function onTreeContext(item: UiTreeItem, event: MouseEvent) {
  if (item.depth === 0) {
    const connection = db.connections.value.find((entry) => entry.id === item.id)
    if (connection) openMenu(event, { connection })
  } else {
    openMenu(event, { item })
  }
}
</script>

<template>
  <aside
    class="relative flex shrink-0 flex-col border-r border-border dark:border-border-dark"
    :style="{ width: `${sidebarWidth}px` }"
  >
    <div class="shrink-0 space-y-[8px] px-[10px] py-[10px]">
      <UiSearchInput v-model="db.keyword.value" size="sm" placeholder="搜索连接…" />
      <UiButton variant="secondary" size="sm" block @click="emit('newConnection')"
        >+ 新建连接</UiButton
      >
    </div>
    <UiScrollArea as-child axis="vertical">
      <div class="min-h-0 flex-1 px-[4px] pb-[8px]">
        <UiTree
          v-model="db.selectedResource.value"
          :items="db.visibleTreeItems.value"
          :row-height="24"
          @update:model-value="onTreeSelect"
          @toggle="db.toggleTree"
          @open="onTreeOpen"
          @contextmenu="onTreeContext"
        >
          <template #icon="{ item }">
            <img
              v-if="item.depth === 0"
              :src="
                DB_TYPE_META[
                  db.connections.value.find((connection) => connection.id === item.id)
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
              <UiTooltip
                v-else
                :content="
                  db.connectError.value[item.id] ||
                  (db.connections.value.find((connection) => connection.id === item.id)?.status ===
                  'online'
                    ? '已连接'
                    : '未连接')
                "
              >
                <span
                  class="h-[6px] w-[6px] shrink-0 rounded-full"
                  :class="
                    db.connectError.value[item.id]
                      ? 'bg-danger-strong dark:bg-danger-dark'
                      : db.connections.value.find((connection) => connection.id === item.id)
                            ?.status === 'online'
                        ? 'bg-success-strong dark:bg-success-dark'
                        : 'bg-text-muted/40 dark:bg-text-muted-dark/40'
                  "
                />
              </UiTooltip>
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
    </UiScrollArea>
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
    <CreateDatabaseDialog :db="db" :connection="createDbFor" @close="createDbFor = null" />
    <ConfirmDialog
      :open="dropDbTarget !== null"
      title="删除数据库"
      :message="`确定删除数据库「${dropDbTarget?.name ?? ''}」吗？库内所有表与数据将被删除，不可恢复。`"
      confirm-label="删除"
      danger
      @close="dropDbTarget = null"
      @confirm="confirmDropDb"
    />
    <UiModal
      :open="tableAction?.mode === 'rename'"
      title="重命名表"
      size="sm"
      @close="tableAction = null"
    >
      <UiInput
        v-model="renameValue"
        size="sm"
        placeholder="新表名"
        @keydown.enter="confirmTableAction"
      />
      <template #footer>
        <UiButton size="sm" variant="ghost" @click="tableAction = null">取消</UiButton>
        <UiButton
          size="sm"
          variant="primary"
          :disabled="!renameValue.trim()"
          @click="confirmTableAction"
          >确定</UiButton
        >
      </template>
    </UiModal>
    <ConfirmDialog
      :open="tableAction?.mode === 'truncate'"
      title="清空表"
      :message="`确定清空「${tableAction?.name ?? ''}」吗？表内所有数据将被删除，不可恢复。`"
      confirm-label="清空"
      danger
      @close="tableAction = null"
      @confirm="confirmTableAction"
    />
    <ConfirmDialog
      :open="tableAction?.mode === 'drop'"
      title="删除表"
      :message="`确定删除表「${tableAction?.name ?? ''}」吗？表结构与数据将被删除，不可恢复。`"
      confirm-label="删除"
      danger
      @close="tableAction = null"
      @confirm="confirmTableAction"
    />
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
