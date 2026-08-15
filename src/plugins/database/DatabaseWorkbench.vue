<script setup lang="ts">
/**
 * database 工作台主容器（状态中枢 + 三栏布局）
 * 左栏 ConnectionsSidebar（连接树）→ 中栏页签区（SQL 编辑器/数据/结构/Redis 键）→
 * 右栏 InspectorPanel（概览/历史/收藏）；所有状态在 useDatabase。
 * 页签语义：SQL 编辑器是可保存的工作区（未保存灰点 / 已保存绿点）；右键可改别名。
 */
import { computed, onMounted, ref } from 'vue'
import { UiButton, UiIconButton, UiInput, UiModal, UiTabs } from '@/core/ui'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import { useDatabase } from './useDatabase'
import { useSplitPane } from './useSplitPane'
import ConnectionsSidebar from './ConnectionsSidebar.vue'
import QueryTab from './QueryTab.vue'
import DataTab from './DataTab.vue'
import StructureTab from './StructureTab.vue'
import RedisTab from './RedisTab.vue'
import InspectorPanel from './InspectorPanel.vue'
import ConnectionDialog from './ConnectionDialog.vue'
import type { ConnConfig, DbConnectionInfo } from './contracts'

const db = useDatabase()
const inspectorOpen = ref(true)

/** 右侧摘要宽度（默认 220，可拖拽；不持久化） */
const inspectorSplit = useSplitPane({ initial: 220, min: 160, max: 480 })
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

/** 页签右键菜单（重命名/关闭） */
const tabMenu = ref<{ x: number; y: number; tabId: string } | null>(null)
const renameDialogOpen = ref(false)
const renameValue = ref('')

const tabMenuItems = computed<ContextMenuItem[]>(() => {
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
    x: Math.min(mouse.clientX, window.innerWidth - 132),
    y: Math.min(mouse.clientY, window.innerHeight - 120),
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
const editorMenuItems = computed<ContextMenuItem[]>(() => {
  const saved = db.savedSql.value
  const items: ContextMenuItem[] = saved.length
    ? saved.map((entry) => ({
        label: entry.title || '未命名',
        onClick: () => db.applySaved(entry),
      }))
    : []
  return items
})

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
      <!-- 独立工具栏：不随页签存在与否显示/隐藏 -->
      <div
        class="flex h-[32px] shrink-0 items-center gap-[2px] border-b border-border px-[6px] dark:border-border-dark"
      >
        <UiButton
          size="xs"
          variant="ghost"
          title="新建 SQL 编辑器（Ctrl+S 保存）"
          class="shrink-0"
          @click="db.openSqlEditor()"
        >
          打开SQL编辑器
        </UiButton>
        <UiIconButton
          v-if="db.savedSql.value.length"
          label="打开已保存的 SQL 编辑器"
          size="xs"
          class="shrink-0"
          @click="openEditorMenu($event)"
        >
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <path d="m6 9 6 6 6-6" />
          </svg>
        </UiIconButton>
        <UiTabs
          v-if="db.tabs.value.length"
          :model-value="db.activeTabId.value"
          :items="tabItems"
          variant="line"
          size="sm"
          class="min-w-0 flex-1"
          @update:model-value="(v) => (db.activeTabId.value = String(v))"
          @close="db.closeTab"
          @contextmenu="onTabContext"
        />
      </div>

      <div
        v-if="!db.tabs.value.length"
        class="grid flex-1 place-items-center text-text-muted dark:text-text-muted-dark"
      >
        <div class="text-center">
          <p class="mb-[8px] text-h2">从左侧选择一个连接开始</p>
          <p class="mb-[16px] text-body-sm">或点击「+ 新建连接」添加数据库</p>
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
      <template v-else-if="db.activeTabKind.value === 'redis'">
        <RedisTab :db="db" />
      </template>
    </main>

    <!-- 中栏/摘要分隔条（可拖拽） -->
    <div
      v-if="inspectorOpen && db.tabs.value.length"
      class="w-[5px] shrink-0 cursor-col-resize bg-surface-muted transition-colors hover:bg-tertiary/40 dark:bg-surface-muted-dark"
      title="拖拽调整摘要宽度"
      @mousedown="(e) => inspectorSplit.onPointerDown(e)"
    />

    <!-- 右栏：摘要 -->
    <aside
      v-if="inspectorOpen && db.tabs.value.length"
      :style="{ width: `${inspectorSplit.size.value}px` }"
      class="flex shrink-0 flex-col border-l border-border dark:border-border-dark"
    >
      <div
        class="flex h-[28px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
      >
        <span class="text-caption font-semibold text-primary dark:text-primary-dark">摘要</span>
        <UiIconButton label="收起摘要" size="xs" class="ml-auto" @click="inspectorOpen = false">
          <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M8 6l6 6-6 6M13 6l6 6-6 6" />
          </svg>
        </UiIconButton>
      </div>
      <InspectorPanel :db="db" />
    </aside>

    <!-- 右栏收起态轨道条 -->
    <div
      v-else-if="db.tabs.value.length"
      class="flex w-[28px] shrink-0 flex-col items-center gap-[6px] border-l border-border py-[6px] dark:border-border-dark"
    >
      <UiIconButton label="展开摘要" size="xs" @click="inspectorOpen = true">
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M16 6l-6 6 6 6M11 6l-6 6 6 6" />
        </svg>
      </UiIconButton>
      <span
        class="text-caption text-text-muted [writing-mode:vertical-rl] dark:text-text-muted-dark"
        >摘要</span
      >
    </div>

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
    <ContextMenu
      v-if="tabMenu"
      :x="tabMenu.x"
      :y="tabMenu.y"
      :items="tabMenuItems"
      size="sm"
      @close="tabMenu = null"
    />

    <!-- 重命名页签 -->
    <UiModal :open="renameDialogOpen" title="重命名页签" size="sm" @close="renameDialogOpen = false">
      <UiInput v-model="renameValue" size="sm" placeholder="输入页签名（别名）" @keydown.enter="confirmRename" />
      <template #footer>
        <UiButton size="sm" variant="ghost" @click="renameDialogOpen = false">取消</UiButton>
        <UiButton size="sm" variant="primary" @click="confirmRename">确定</UiButton>
      </template>
    </UiModal>

    <!-- 已保存 SQL 编辑器下拉 -->
    <ContextMenu
      v-if="editorMenu"
      :x="editorMenu.x"
      :y="editorMenu.y"
      :items="editorMenuItems"
      size="sm"
      @close="editorMenu = null"
    />
  </div>
</template>
