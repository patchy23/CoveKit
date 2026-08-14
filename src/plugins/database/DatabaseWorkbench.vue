<script setup lang="ts">
/**
 * database 工作台主容器（状态中枢 + 三栏布局）
 * 左栏 ConnectionsSidebar（连接树）→ 中栏页签区（查询/数据/结构/Redis 键）→
 * 右栏 InspectorPanel（概览/历史/收藏）；所有状态在 useDatabase。
 */
import { computed, onMounted, ref } from 'vue'
import { UiIconButton, UiTabs } from '@/core/ui'
import { useDatabase } from './useDatabase'
import ConnectionsSidebar from './ConnectionsSidebar.vue'
import QueryTab from './QueryTab.vue'
import DataTab from './DataTab.vue'
import StructureTab from './StructureTab.vue'
import RedisTab from './RedisTab.vue'
import InspectorPanel from './InspectorPanel.vue'
import ConnectionDialog from './ConnectionDialog.vue'
import type { ConnConfig } from './contracts'

const db = useDatabase()
const inspectorOpen = ref(true)
const dialogOpen = ref(false)
const editingConnection = ref<ConnConfig | null>(null)

const tabItems = computed(() =>
  db.tabs.value.map((tab) => ({
    value: tab.id,
    label: tab.label,
    closable: true,
    status:
      db.tabContexts.value[tab.id]?.connectionId &&
      db.connections.value.find((c) => c.id === db.tabContexts.value[tab.id]?.connectionId)?.status ===
        'online'
        ? ('success' as const)
        : ('danger' as const),
  }))
)

function onNewConnection() {
  editingConnection.value = null
  dialogOpen.value = true
}

function onSaved(config: ConnConfig) {
  // 保存成功后立即连接（无真实数据库时错误在树节点展示）
  const conn = db.connections.value.find((c) => c.id === config.id)
  if (conn) void db.connect(conn).catch(() => {})
}

onMounted(() => {
  void db.refreshConnections()
  void db.refreshHistory()
  void db.refreshSaved()
})
</script>

<template>
  <div class="flex h-full min-h-0 w-full">
    <ConnectionsSidebar :db="db" @new-connection="onNewConnection" />

    <!-- 中栏：页签工作区 -->
    <main class="flex min-h-0 min-w-0 flex-1 flex-col">
      <UiTabs
        v-if="tabItems.length"
        :model-value="db.activeTabId.value"
        :items="tabItems"
        variant="line"
        size="sm"
        @update:model-value="(v) => (db.activeTabId.value = String(v))"
        @close="db.closeTab"
      />

      <div
        v-if="!db.tabs.value.length"
        class="grid flex-1 place-items-center text-text-muted dark:text-text-muted-dark"
      >
        <div class="text-center">
          <p class="mb-[8px] text-h2">从左侧选择一个连接开始</p>
          <p class="text-body-sm">或点击「+ 新建连接」添加数据库</p>
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

    <!-- 右栏：摘要 -->
    <aside
      v-if="inspectorOpen && db.tabs.value.length"
      class="flex w-[220px] shrink-0 flex-col border-l border-border dark:border-border-dark"
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
  </div>
</template>
