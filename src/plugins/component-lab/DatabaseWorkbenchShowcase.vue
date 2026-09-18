<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { computed, ref } from 'vue'
import {
  UiBadge,
  UiButton,
  UiDataGrid,
  UiIconButton,
  UiCodeEditor,
  UiSearchInput,
  UiSelect,
  UiSplitPane,
  UiStatusBar,
  UiTabs,
  UiToolbar,
  UiTree,
  UiWorkbenchTabs,
} from '@/core/ui'
import type { UiTreeItem, UiWorkbenchTab } from '@/core/ui'
import {
  columns,
  connectionTabs,
  createDocuments,
  createTreeItems,
  databaseOptions,
  limitOptions,
  resultTabs,
  rows,
  schemaOptions,
} from './databaseShowcaseData'

const leftWidth = ref(224)
const mainWidth = ref(610)
const editorHeight = ref(212)
const keyword = ref('')
const selectedResource = ref('table-users')
const activeConnection = ref('mysql-prod')
const activeDocument = ref('query-1')
const activeResult = ref('data')
const selectedRow = ref('1')
const database = ref('covekit')
const schema = ref('public')
const rowLimit = ref('1000')
const sql = ref(`SELECT id, username, display_name, email, status, created_at
FROM users
WHERE status = 'active'
ORDER BY created_at DESC
LIMIT 1000;`)

const documents = ref<UiWorkbenchTab[]>(createDocuments())
const treeItems = ref<UiTreeItem[]>(createTreeItems())
const visibleTreeItems = computed(() => {
  const value = keyword.value.trim().toLocaleLowerCase()
  if (value) {
    const kept = new Set<number>()
    treeItems.value.forEach((item, index) => {
      if (!item.label.toLocaleLowerCase().includes(value)) return
      kept.add(index)
      let depth = item.depth - 1
      for (let parentIndex = index - 1; parentIndex >= 0 && depth >= 0; parentIndex -= 1) {
        if (treeItems.value[parentIndex].depth !== depth) continue
        kept.add(parentIndex)
        depth -= 1
      }
    })
    return treeItems.value.filter((_, index) => kept.has(index))
  }

  const visible: UiTreeItem[] = []
  let collapsedDepth = Number.POSITIVE_INFINITY
  for (const item of treeItems.value) {
    if (item.depth <= collapsedDepth) collapsedDepth = Number.POSITIVE_INFINITY
    if (item.depth >= collapsedDepth) continue
    visible.push(item)
    if (item.expandable && !item.expanded) collapsedDepth = item.depth + 1
  }
  return visible
})
function closeDocument(id: string) {
  documents.value = documents.value.filter((item) => item.id !== id)
  if (activeDocument.value === id) activeDocument.value = documents.value[0]?.id ?? ''
}
function toggleTree(item: UiTreeItem) {
  item.expanded = !item.expanded
}
</script>

<template>
  <section class="space-y-3">
    <div class="flex flex-wrap items-start justify-between gap-3">
      <div>
        <h2 class="text-h2 text-primary dark:text-primary-dark">数据库工作台 UI 模型</h2>
      </div>
      <div class="flex gap-2">
        <UiBadge tone="info">交互模型</UiBadge><UiBadge tone="neutral">无真实连接</UiBadge>
      </div>
    </div>

    <div
      class="flex h-[650px] flex-col overflow-hidden rounded-xl border border-border bg-surface shadow-sm dark:border-border-dark dark:bg-surface-dark"
    >
      <div
        class="flex items-center border-b border-border bg-surface-muted px-sm dark:border-border-dark dark:bg-surface-muted-dark"
      >
        <UiTabs v-model="activeConnection" :items="connectionTabs" size="xs" />
        <UiIconButton class="ml-auto" label="新建数据库连接" size="xs">＋</UiIconButton>
      </div>
      <UiSplitPane v-model="leftWidth" class="min-h-0 flex-1" :min="180" :max="320">
        <template #primary>
          <aside class="flex h-full min-w-0 flex-col bg-surface-muted dark:bg-surface-muted-dark">
            <div class="space-y-1.5 border-b border-border p-sm dark:border-border-dark">
              <UiSearchInput v-model="keyword" size="xs" placeholder="筛选数据库对象" />
              <div class="flex gap-1">
                <UiButton class="flex-1" size="xs" variant="secondary">＋ 新建</UiButton
                ><UiIconButton label="刷新对象树" size="xs">↻</UiIconButton
                ><UiIconButton label="更多操作" size="xs">•••</UiIconButton>
              </div>
            </div>
            <UiTree
              v-model="selectedResource"
              class="min-h-0 flex-1"
              :items="visibleTreeItems"
              :row-height="24"
              @toggle="toggleTree"
            />
          </aside>
        </template>
        <template #secondary>
          <UiSplitPane v-model="mainWidth" class="h-full" :min="480" :max="900">
            <template #primary>
              <main class="flex h-full min-w-0 flex-col">
                <UiWorkbenchTabs
                  v-model="activeDocument"
                  :items="documents"
                  @close="closeDocument"
                />
                <UiSplitPane
                  v-model="editorHeight"
                  class="min-h-0 flex-1"
                  direction="vertical"
                  :min="140"
                  :max="390"
                >
                  <template #primary>
                    <div class="flex h-full min-h-0 flex-col">
                      <UiToolbar density="compact">
                        <UiButton size="xs" variant="primary">▶ 运行</UiButton
                        ><UiIconButton label="停止执行" size="xs">■</UiIconButton
                        ><UiIconButton label="格式化 SQL" size="xs">{ }</UiIconButton>
                        <span class="mx-1 h-4 w-px bg-border dark:bg-border-dark" />
                        <UiSelect
                          v-model="database"
                          class="w-32"
                          :options="databaseOptions"
                          size="xs"
                        /><UiSelect
                          v-model="schema"
                          class="w-24"
                          :options="schemaOptions"
                          size="xs"
                        /><UiSelect
                          v-model="rowLimit"
                          class="w-24"
                          :options="limitOptions"
                          size="xs"
                        />
                        <span class="ml-auto text-caption text-text-muted dark:text-text-muted-dark"
                          >Ctrl + Enter</span
                        >
                      </UiToolbar>
                      <UiCodeEditor
                        v-model="sql"
                        class="min-h-0 flex-1 rounded-none border-0"
                        mode="minimal"
                        rounded-none
                        border-0
                      />
                    </div>
                  </template>
                  <template #secondary>
                    <div class="flex h-full min-h-0 flex-col">
                      <div
                        class="flex items-center border-b border-border px-2 dark:border-border-dark"
                      >
                        <UiTabs v-model="activeResult" :items="resultTabs" size="xs" /><span
                          class="ml-auto text-caption text-text-muted dark:text-text-muted-dark"
                          >48 行 · 36 ms</span
                        >
                      </div>
                      <UiDataGrid
                        v-model="selectedRow"
                        class="min-h-0 flex-1"
                        :columns="columns"
                        :rows="rows"
                        row-key="id"
                        height="100%"
                      >
                        <template #cell-status="{ value }"
                          ><UiBadge :tone="value === 'active' ? 'success' : 'danger'" size="sm">{{
                            value
                          }}</UiBadge></template
                        >
                      </UiDataGrid>
                    </div>
                  </template>
                </UiSplitPane>
              </main>
            </template>
            <template #secondary>
              <UiScrollArea as-child axis="both">
                <aside class="h-full min-w-0 bg-surface-muted dark:bg-surface-muted-dark">
                  <div class="border-b border-border px-md py-sm dark:border-border-dark">
                    <div class="text-body font-medium text-primary dark:text-primary-dark">
                      users
                    </div>
                    <div
                      class="mt-0.5 font-mono text-caption text-text-muted dark:text-text-muted-dark"
                    >
                      covekit.public
                    </div>
                  </div>
                  <div class="space-y-4 p-3 text-body-sm">
                    <div>
                      <div class="mb-2 text-label-caps text-text-muted dark:text-text-muted-dark">
                        选中字段
                      </div>
                      <dl class="grid grid-cols-[76px_1fr] gap-x-2 gap-y-1.5">
                        <dt class="text-text-muted dark:text-text-muted-dark">字段名</dt>
                        <dd class="font-mono">username</dd>
                        <dt class="text-text-muted dark:text-text-muted-dark">类型</dt>
                        <dd class="font-mono">varchar(64)</dd>
                        <dt class="text-text-muted dark:text-text-muted-dark">可空</dt>
                        <dd>否</dd>
                        <dt class="text-text-muted dark:text-text-muted-dark">默认值</dt>
                        <dd class="font-mono">NULL</dd>
                      </dl>
                    </div>
                    <div>
                      <div class="mb-2 text-label-caps text-text-muted dark:text-text-muted-dark">
                        索引
                      </div>
                      <div class="space-y-1.5">
                        <div
                          v-for="item in [
                            'users_pkey · PRIMARY · id',
                            'users_email_key · UNIQUE · email',
                          ]"
                          :key="item"
                          class="rounded-md border border-border bg-surface px-2 py-1.5 font-mono text-caption dark:border-border-dark dark:bg-surface-dark"
                        >
                          {{ item }}
                        </div>
                      </div>
                    </div>
                  </div>
                </aside>
              </UiScrollArea>
            </template>
          </UiSplitPane>
        </template>
      </UiSplitPane>
      <UiStatusBar tone="success"
        ><span>MySQL 8.0.36</span><span>事务：自动提交</span
        ><span class="ml-auto">UTF-8 · Asia/Shanghai · 就绪</span></UiStatusBar
      >
    </div>
  </section>
</template>
