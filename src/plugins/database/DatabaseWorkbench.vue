<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  UiButton,
  UiIcon,
  UiIconButton,
  UiInput,
  UiModal,
  UiTabs,
  UiTabsOverflowMenu,
  UiToolbar,
} from '@/core/ui'
import { UiContextMenu, type UiContextMenuItem } from '@/core/ui'
import { useTabsOverflow } from '@/core/ui/useTabsOverflow'
import { useDatabase } from './useDatabase'
import { useSettingsStore } from '@/stores/settings'
import { useSplitPane } from '@/core/ui/useSplitPane'
import ConnectionsSidebar from './ConnectionsSidebar.vue'
import QueryTab from './QueryTab.vue'
import DataTab from './DataTab.vue'
import StructureTab from './StructureTab.vue'
import CreateTableTab from './CreateTableTab.vue'
import RedisTab from './RedisTab.vue'
import DatabaseInspectorPane from './DatabaseInspectorPane.vue'
import ConnectionDialog from './ConnectionDialog.vue'
import type { ConnConfig, DbConnectionInfo } from './contracts'
import { useDatabaseToolLifecycle } from './toolLifecycle'

const db = useDatabase()
// 工具资源生命周期：关闭页签/退出时断开全部数据库连接（T10-4）
useDatabaseToolLifecycle()
const settings = useSettingsStore()
const defaultInspectorOpen = typeof window !== 'undefined' && window.innerWidth >= 1200
const inspectorOpen = computed(() =>
  settings.getToolSetting<boolean>('database', 'inspectorOpen', defaultInspectorOpen)
)
function setInspectorOpen(open: boolean) {
  void settings.setToolSetting('database', 'inspectorOpen', open).catch(db.showError)
}

/** 右侧摘要宽度（默认 220，可拖拽；不持久化） */
const inspectorSplit = useSplitPane({ initial: 220, min: 160, max: 480, reverse: true })
const dialogOpen = ref(false)
const editingConnection = ref<ConnConfig | null>(null)

/** 页签：SQL 编辑器显示保存状态（未保存灰点/已保存绿点），其余页签无状态点 */
const tabItems = computed(() =>
  db.tabs.value.map((tab) => {
    const isQuery = tab.kind === 'query'
    const state = db.queryStates.value[tab.id]
    const unsaved = !state?.savedId || state.dirty
    return {
      value: tab.id,
      label: tab.label,
      closable: true,
      status: isQuery ? (unsaved ? ('neutral' as const) : ('success' as const)) : undefined,
      statusTitle: isQuery ? (unsaved ? '未保存' : '已保存') : undefined,
    }
  })
)

/** 页签条溢出：按容器宽度切分可见/收纳（只为容器内的溢出触发器预留宽度） */
const tabBarRef = ref<HTMLElement | null>(null)
const activeTabValue = computed(() => db.activeTabId.value)
const { visibleItems: visibleTabs, hiddenItems: hiddenTabs } = useTabsOverflow(
  tabBarRef,
  tabItems,
  activeTabValue,
  28
)

/** 页签右键菜单（重命名/关闭） */
const tabMenu = ref<{ x: number; y: number; tabId: string } | null>(null)
const renameDialogOpen = ref(false)
const renameValue = ref('')

const tabMenuItems = computed<UiContextMenuItem[]>(() => {
  if (!tabMenu.value) return []
  const tab = db.tabs.value.find((t) => t.id === tabMenu.value?.tabId)
  return [
    {
      label: '重命名',
      onClick: () => {
        renameValue.value = tab?.label ?? ''
        renameDialogOpen.value = true
      },
    },
    { label: '关闭', onClick: () => db.closeTab(tabMenu.value!.tabId) },
  ]
})

function onTabContext(value: string, mouse: MouseEvent) {
  tabMenu.value = {
    x: mouse.clientX,
    y: mouse.clientY,
    tabId: value,
  }
}

function confirmRename() {
  db.renameActiveTab(renameValue.value)
  renameDialogOpen.value = false
}

/** 打开SQL编辑器：新建；下拉可选择已保存的编辑器（复用收藏） */
const editorMenu = ref<{ x: number; y: number } | null>(null)
function openEditorMenu(event: MouseEvent) {
  editorMenu.value = { x: event.clientX, y: event.clientY + 4 }
}
const editorMenuItems = computed<UiContextMenuItem[]>(() => {
  const saved = db.savedSql.value
  const items: UiContextMenuItem[] = saved.length
    ? saved.map((entry) => ({
        label: entry.title || '未命名',
        onClick: () => db.applySaved(entry),
      }))
    : []
  return items
})

/** 新建查询继承当前页签的连接与库范围。 */
function newSqlQuery() {
  const ctx = db.activeTabContext.value
  db.openSqlEditorWithSql(ctx.connectionId, '', ctx.database, ctx.schema)
}

function onNewConnection() {
  editingConnection.value = null
  dialogOpen.value = true
}

function onEditConnection(connection: DbConnectionInfo) {
  const {
    id,
    label,
    dbType,
    host,
    port,
    username,
    database,
    env,
    readonly,
    ssl,
    connectTimeoutMs,
  } = connection
  editingConnection.value = {
    id,
    label,
    dbType,
    host,
    port,
    username,
    database,
    env,
    readonly,
    ssl,
    connectTimeoutMs,
  }
  dialogOpen.value = true
}

async function onSaved(config: ConnConfig) {
  // 只刷新列表（保存后列表/右键菜单/编辑预填必须是最新配置）；
  // 不自动连接——用户双击左侧列表再连接
  await db.refreshConnections()
  void config
}

onMounted(() => {
  void db.refreshConnections()
  void db.refreshHistory()
  void db.refreshSaved()
})
</script>

<template>
  <div class="flex h-full min-h-0 w-full">
    <ConnectionsSidebar
      :db="db"
      @new-connection="onNewConnection"
      @edit-connection="onEditConnection"
    />

    <!-- 中栏：页签工作区 -->
    <main class="flex min-h-0 min-w-0 flex-1 flex-col">
      <!-- 页签末端的新建入口固定可见，不重复扣减按钮宽度。 -->
      <UiToolbar density="compact" bordered>
        <div ref="tabBarRef" class="flex min-w-0 flex-1 items-center overflow-hidden">
          <UiTabs
            v-if="db.tabs.value.length"
            :model-value="db.activeTabId.value"
            :items="visibleTabs"
            variant="line"
            size="xs"
            class="min-w-0 flex-1"
            @update:model-value="(v) => (db.activeTabId.value = String(v))"
            @close="db.closeTab"
            @contextmenu="onTabContext"
          />
          <UiTabsOverflowMenu
            v-if="hiddenTabs.length"
            :items="hiddenTabs"
            :model-value="db.activeTabId.value"
            @select="(v) => (db.activeTabId.value = v)"
            @close="db.closeTab"
          />
        </div>
        <template #trailing
          ><UiIconButton label="新建 SQL 查询" size="xs" class="shrink-0" @click="newSqlQuery">
            <UiIcon name="plus" :size="14" />
          </UiIconButton>
          <UiIconButton
            v-if="db.savedSql.value.length"
            label="打开已保存的 SQL"
            size="xs"
            class="shrink-0"
            @click="openEditorMenu"
          >
            <UiIcon name="chevron-down" :size="12" /> </UiIconButton
        ></template>
      </UiToolbar>

      <div
        v-if="!db.tabs.value.length"
        class="grid flex-1 place-items-center text-text-muted dark:text-text-muted-dark"
      >
        <div class="text-center">
          <p class="mb-[8px] text-h2">从左侧选择一个连接开始</p>
          <p class="mb-[16px] text-body-sm">或点击左侧搜索框旁的「＋」添加连接</p>
          <UiButton size="sm" variant="secondary" @click="db.openSqlEditor()">
            打开SQL编辑器
          </UiButton>
        </div>
      </div>

      <template v-else-if="db.activeTabKind.value === 'query'">
        <QueryTab :db="db" />
      </template>
      <template v-else-if="db.activeTabKind.value === 'data'">
        <DataTab :db="db" />
      </template>
      <template v-else-if="db.activeTabKind.value === 'structure'">
        <StructureTab :db="db" />
      </template>
      <template v-else-if="db.activeTabKind.value === 'create-table'">
        <CreateTableTab :db="db" />
      </template>
      <template v-else-if="db.activeTabKind.value === 'redis'">
        <RedisTab :db="db" />
      </template>
    </main>

    <DatabaseInspectorPane
      :db="db"
      :open="inspectorOpen"
      :width="inspectorSplit.size.value"
      @resize="inspectorSplit.onPointerDown"
      @update:open="setInspectorOpen"
    />

    <!-- 提示条 -->
    <div
      v-if="db.errorHint.value"
      class="pointer-events-none absolute bottom-[20px] left-1/2 z-10 -translate-x-1/2 rounded-md border border-border bg-surface px-[10px] py-[4px] text-caption text-primary shadow-md dark:border-border-dark dark:bg-surface-dark dark:text-primary-dark"
    >
      {{ db.errorHint.value }}
    </div>

    <!-- 新建/编辑连接对话框 -->
    <ConnectionDialog
      :open="dialogOpen"
      :editing="editingConnection"
      @close="dialogOpen = false"
      @saved="onSaved"
    />

    <!-- 页签右键菜单 -->
    <UiContextMenu
      v-if="tabMenu"
      :x="tabMenu.x"
      :y="tabMenu.y"
      :items="tabMenuItems"
      size="sm"
      @close="tabMenu = null"
    />

    <!-- 重命名页签 -->
    <UiModal
      :open="renameDialogOpen"
      title="重命名页签"
      size="sm"
      @close="renameDialogOpen = false"
    >
      <UiInput
        v-model="renameValue"
        size="sm"
        placeholder="输入页签名（别名）"
        @keydown.enter="confirmRename"
      />
      <template #footer>
        <UiButton size="sm" variant="ghost" @click="renameDialogOpen = false">取消</UiButton>
        <UiButton size="sm" variant="primary" @click="confirmRename">确定</UiButton>
      </template>
    </UiModal>

    <!-- 已保存 SQL 编辑器下拉 -->
    <UiContextMenu
      v-if="editorMenu"
      :x="editorMenu.x"
      :y="editorMenu.y"
      :items="editorMenuItems"
      size="sm"
      @close="editorMenu = null"
    />
  </div>
</template>
