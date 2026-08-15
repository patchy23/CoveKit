<script setup lang="ts">
/**
 * 左侧连接栏：搜索 + 新建连接 + 对象树（懒加载元数据）
 * 交互约定：未连接节点无折叠箭头，单击仅选中，双击连接并展开；
 * 已连接节点双击折叠/展开（不重新连接）；错误提示走状态点 tooltip，名字始终完整。
 */
import { computed, ref } from 'vue'
import { UiButton, UiIconButton, UiSearchInput, UiSpinner, UiTree } from '@/core/ui'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import DbObjectIcon from './DbObjectIcon.vue'
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

const menu = ref<{ x: number; y: number; connection: DbConnectionInfo } | null>(null)
const deleteTarget = ref<DbConnectionInfo | null>(null)

/** 侧栏宽度（可拖拽，默认 200，范围 168–340） */
const sidebarWidth = ref(200)
let dragging = false

function onDragStart(event: MouseEvent) {
  event.preventDefault()
  dragging = true
  const onMove = (move: MouseEvent) => {
    if (!dragging) return
    sidebarWidth.value = Math.min(340, Math.max(168, move.clientX - 12))
  }
  const onUp = () => {
    dragging = false
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

const menuItems = computed<ContextMenuItem[]>(() => {
  const connection = menu.value?.connection
  if (!connection) return []
  const online = connection.status === 'online'
  return [
    {
      label: online ? '断开连接' : '连接',
      onClick: () => {
        if (online) void db.disconnect(connection)
        else void db.connect(connection).catch(() => {})
      },
    },
    { label: '编辑连接', onClick: () => emit('editConnection', connection) },
    { label: '打开SQL编辑器', onClick: () => db.openSqlEditor(connection.id) },
    { label: '刷新元数据', onClick: () => void db.ensureMeta(connection.id) },
    { label: '', separator: true },
    { label: '删除连接', danger: true, onClick: () => (deleteTarget.value = connection) },
  ]
})

function openMenu(event: MouseEvent, connection: DbConnectionInfo) {
  event.preventDefault()
  menu.value = {
    x: Math.min(event.clientX, window.innerWidth - 132),
    y: Math.min(event.clientY, window.innerHeight - 158),
    connection,
  }
}

function confirmDelete() {
  if (deleteTarget.value) {
    void db.removeConnection(deleteTarget.value.id)
    deleteTarget.value = null
  }
}

function onTreeSelect(id: string) {
  const item = db.visibleTreeItems.value.find((node) => node.id === id)
  if (!item) return
  // 连接节点单击仅选中；连接/展开/新建页签分别由双击与菜单承担
  if (item.depth === 0) return
  if (item.kind === 'table' || item.kind === 'view' || item.kind === 'materialized_view' || item.kind === 'key') {
    void db.selectResource(id)
  }
}

function onTreeContext(mouse: MouseEvent, item: { id: string; depth: number }) {
  if (item.depth === 0) {
    const connection = db.connections.value.find((c) => c.id === item.id)
    if (connection) openMenu(mouse, connection)
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
        @context="onTreeContext"
      >
        <template #icon="{ item }">
          <img
            v-if="item.depth === 0"
            :src="DB_TYPE_META[db.connections.value.find((c) => c.id === item.id)?.dbType as keyof typeof DB_TYPE_META]?.icon ?? ''"
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
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <path d="M18 6 6 18M6 6l12 12" />
                </svg>
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
