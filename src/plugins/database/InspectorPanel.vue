<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * 右侧摘要面板：概览（连接信息 + 当前表）+ 历史 + 收藏
 */
import { computed, ref } from 'vue'
import { UiBadge, UiButton, UiIcon, UiIconButton, UiTable, UiTableCell, UiTabs } from '@/core/ui'
import { UiConfirmDialog } from '@/core/ui'
import { DB_TYPE_META } from './useDatabaseMeta'
import type { HistoryEntry, SavedEntry } from './contracts'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props

const section = ref<'overview' | 'history' | 'saved'>('overview')

const conn = computed(() => db.activeTabConnection.value)

/** 收藏删除确认（提示不可恢复） */
const deleteTarget = ref<SavedEntry | null>(null)

function confirmDeleteSaved() {
  if (deleteTarget.value) {
    void db.removeSaved(deleteTarget.value.id)
    deleteTarget.value = null
  }
}

function onHistory(entry: HistoryEntry) {
  db.applyHistory(entry)
  db.showError('已恢复到当前查询')
}

function onSaved(entry: SavedEntry) {
  db.applySaved(entry)
  db.showError(`已打开「${entry.title}」`)
}

/** 连接时间展示（epoch 秒 → 本地时间字符串） */
function formatConnectedAt(epochSeconds: number): string {
  if (!epochSeconds) return '—'
  const d = new Date(epochSeconds * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}
</script>

<template>
  <!-- 区块切换 -->
  <UiTabs
    v-model="section"
    :items="[
      { value: 'overview', label: '概览' },
      { value: 'history', label: '历史', badge: db.history.value.length },
      { value: 'saved', label: '收藏', badge: db.savedSql.value.length },
    ]"
    variant="line"
    size="xs"
    class="shrink-0 border-b border-border dark:border-border-dark"
  />

  <UiScrollArea as-child axis="both">
    <div class="min-h-0 flex-1 p-[8px]">
      <!-- 概览 -->
      <div v-if="section === 'overview'" class="space-y-[8px]">
        <section v-if="conn">
          <div class="mb-[4px] text-label-caps text-text-muted dark:text-text-muted-dark">连接</div>
          <div class="space-y-[3px] rounded border border-border p-[6px] dark:border-border-dark">
            <div class="truncate text-body-sm font-medium text-primary dark:text-primary-dark">
              {{ conn.label }}
            </div>
            <div class="truncate font-mono text-caption text-secondary dark:text-secondary-dark">
              {{ conn.host }}
            </div>
            <div class="flex items-center gap-[6px] text-caption">
              <span class="text-text-muted dark:text-text-muted-dark">状态</span>
              <UiBadge :tone="conn.status === 'online' ? 'success' : 'danger'" size="xs">
                {{ conn.status === 'online' ? '在线' : '离线' }}
              </UiBadge>
            </div>
            <div v-if="conn.version" class="flex items-center gap-[6px] text-caption">
              <span class="text-text-muted dark:text-text-muted-dark">版本</span>
              <span class="font-mono text-primary dark:text-primary-dark">{{ conn.version }}</span>
              <span v-if="conn.latencyMs" class="ml-auto text-text-muted dark:text-text-muted-dark"
                >{{ conn.latencyMs }} ms</span
              >
            </div>
            <div class="flex items-center gap-[6px] text-caption">
              <span class="text-text-muted dark:text-text-muted-dark">权限</span>
              <UiBadge :tone="conn.readonly ? 'warning' : 'success'" size="xs">
                {{ conn.readonly ? '只读' : '可写' }}
              </UiBadge>
            </div>
            <div v-if="conn.connectedAt > 0" class="flex items-center gap-[6px] text-caption">
              <span class="text-text-muted dark:text-text-muted-dark">连接时间</span>
              <span class="font-mono text-primary dark:text-primary-dark">{{
                formatConnectedAt(conn.connectedAt)
              }}</span>
            </div>
          </div>
        </section>
        <section v-else>
          <div
            class="rounded border border-border p-[8px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
          >
            暂无活动连接
          </div>
        </section>

        <section>
          <div class="mb-[4px] text-label-caps text-text-muted dark:text-text-muted-dark">
            当前页签
          </div>
          <div class="space-y-[3px] rounded border border-border p-[6px] dark:border-border-dark">
            <div class="font-mono text-body-sm font-medium text-primary dark:text-primary-dark">
              {{ db.activeTab.value?.label ?? '—' }}
            </div>
            <div class="flex items-center gap-[6px] text-caption">
              <span class="text-text-muted dark:text-text-muted-dark">类型</span>
              <UiBadge tone="neutral" size="xs">{{
                conn ? (DB_TYPE_META[conn.dbType]?.label ?? conn.dbType) : '—'
              }}</UiBadge>
            </div>
          </div>
        </section>
      </div>

      <!-- 历史 -->
      <div v-else-if="section === 'history'" class="space-y-[2px]">
        <div class="mb-[4px] flex justify-end">
          <UiButton size="xs" variant="ghost" @click="db.clearHistory()">清空</UiButton>
        </div>
        <div
          v-for="entry in db.history.value"
          :key="entry.id"
          class="flex w-full cursor-pointer flex-col gap-[2px] rounded px-[6px] py-[4px] text-left hover:bg-border dark:hover:bg-border-dark"
          @click="onHistory(entry)"
        >
          <div class="flex items-center gap-[4px]">
            <span
              class="h-[5px] w-[5px] rounded-full"
              :class="entry.status === 'success' ? 'bg-success-strong' : 'bg-danger-strong'"
            />
            <span class="text-caption text-text-muted dark:text-text-muted-dark">{{
              entry.at
            }}</span>
            <span class="ml-auto font-mono text-caption text-text-muted dark:text-text-muted-dark"
              >{{ entry.durationMs }} ms</span
            >
          </div>
          <div class="line-clamp-2 font-mono text-caption text-primary dark:text-primary-dark">
            {{ entry.sql }}
          </div>
        </div>
        <p
          v-if="!db.history.value.length"
          class="py-[16px] text-center text-caption text-text-muted dark:text-text-muted-dark"
        >
          暂无查询历史
        </p>
      </div>

      <!-- 收藏 -->
      <div v-else class="space-y-[2px]">
        <div
          v-for="entry in db.savedSql.value"
          :key="entry.id"
          class="group flex items-start gap-[4px] rounded px-[6px] py-[4px] hover:bg-border dark:hover:bg-border-dark"
        >
          <div class="min-w-0 flex-1 cursor-pointer text-left" @click="onSaved(entry)">
            <div class="text-caption font-semibold text-primary dark:text-primary-dark">
              {{ entry.title }}
            </div>
            <div
              class="line-clamp-2 font-mono text-caption text-secondary dark:text-secondary-dark"
            >
              {{ entry.sql }}
            </div>
          </div>
          <UiIconButton
            label="删除"
            size="xs"
            class="shrink-0 opacity-0 group-hover:opacity-100"
            @click.stop="deleteTarget = entry"
          >
            <UiIcon name="x" :size="10" />
          </UiIconButton>
        </div>
        <p
          v-if="!db.savedSql.value.length"
          class="py-[16px] text-center text-caption text-text-muted dark:text-text-muted-dark"
        >
          暂无收藏 SQL
        </p>
      </div>

      <!-- 收藏删除确认 -->
      <UiConfirmDialog
        :open="deleteTarget !== null"
        title="删除收藏"
        :message="`确定删除「${deleteTarget?.title ?? ''}」吗？删除后不可恢复。`"
        confirm-label="删除"
        danger
        @close="deleteTarget = null"
        @confirm="confirmDeleteSaved"
      />
    </div>
  </UiScrollArea>

  <!-- 连接详情小表（概览底部：字段列表） -->
  <UiTable
    v-if="section === 'overview' && conn"
    density="compact"
    :framed="false"
    class="border-t border-border dark:border-border-dark"
  >
    <tbody>
      <tr>
        <UiTableCell>库</UiTableCell>
        <UiTableCell content="technical">{{ conn.database }}</UiTableCell>
      </tr>
      <tr>
        <UiTableCell>环境</UiTableCell>
        <UiTableCell>{{ conn.env }}</UiTableCell>
      </tr>
      <tr v-if="conn.error">
        <UiTableCell>错误</UiTableCell>
        <UiTableCell content="technical">{{ conn.error }}</UiTableCell>
      </tr>
    </tbody>
  </UiTable>
</template>
