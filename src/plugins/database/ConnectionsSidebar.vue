<script setup lang="ts">
/**
 * 左侧连接栏：搜索 + 新建连接 + 对象树（懒加载元数据；离线连接点击即连接）
 */
import { computed, ref } from 'vue'
import { UiButton, UiSearchInput, UiSpinner, UiTree } from '@/core/ui'
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
}>()

const { db } = props

const menu = ref<{ x: number; y: number; connection: DbConnectionInfo } | null>(null)
const deleteTarget = ref<DbConnectionInfo | null>(null)

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
    { label: '新建查询', onClick: () => db.createQuery(connection.id) },
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
  if (item.depth === 0) {
    const conn = db.connections.value.find((c) => c.id === id)
    if (!conn) return
    if (conn.status === 'online') db.createQuery(conn.id)
    else void db.connect(conn).catch(() => {})
    return
  }
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
  <aside class="flex w-[200px] shrink-0 flex-col border-r border-border dark:border-border-dark">
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
          <UiSpinner v-if="item.depth === 0 && db.connecting.value[item.id]" size="xs" />
          <span
            v-else-if="item.depth === 0"
            class="h-[6px] w-[6px] rounded-full"
            :class="
              item.badge === '断开' || db.connectError.value[item.id]
                ? 'bg-danger-strong dark:bg-danger-dark'
                : 'bg-success-strong dark:bg-success-dark'
            "
          />
        </template>
      </UiTree>

      <p
        v-if="!db.visibleTreeItems.value.length"
        class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        {{ db.keyword.value ? '无匹配对象' : '暂无连接，点击上方新建' }}
      </p>
    </div>

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
