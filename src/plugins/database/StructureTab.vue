<script setup lang="ts">
/**
 * 表结构页签：列信息表格（字段/类型/可空/默认值/键/注释）+ 生成查询
 */
import { computed } from 'vue'
import { UiBadge, UiButton, UiTable, UiTableCell } from '@/core/ui'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props

const columns = computed(() => db.structureColumns.value[db.activeTabId.value] ?? [])
const tableName = computed(() => {
  const m = db.activeTabId.value.match(/^structure-(?:[^-]+)-(.*)$/)
  return m?.[1] ?? ''
})

function genQuery() {
  db.openSqlEditor()
  db.patchQueryState({ sql: `SELECT * FROM ${tableName.value} LIMIT 100;` })
}
</script>

<template>
  <div class="min-h-0 flex-1 overflow-auto p-[12px]">
    <div class="mb-[12px] flex items-center justify-between">
      <div>
        <h2 class="text-card-title font-semibold text-primary dark:text-primary-dark">
          {{ tableName }} · 表结构
        </h2>
        <p class="mt-[2px] font-mono text-caption text-secondary dark:text-secondary-dark">
          {{ db.activeTabConnection.value?.label ?? '' }} · {{ columns.length }} 个字段
        </p>
      </div>
      <UiButton size="xs" variant="primary" @click="genQuery">生成查询</UiButton>
    </div>

    <UiTable v-if="columns.length" density="compact" :hoverable="true" :striped="true">
      <thead>
        <tr>
          <UiTableCell as="th">字段</UiTableCell>
          <UiTableCell as="th">类型</UiTableCell>
          <UiTableCell as="th">可空</UiTableCell>
          <UiTableCell as="th">默认值</UiTableCell>
          <UiTableCell as="th">键</UiTableCell>
          <UiTableCell as="th">注释</UiTableCell>
        </tr>
      </thead>
      <tbody>
        <tr v-for="column in columns" :key="column.name">
          <UiTableCell content="technical">{{ column.name }}</UiTableCell>
          <UiTableCell content="technical">{{ column.dataType }}</UiTableCell>
          <UiTableCell>{{ column.nullable }}</UiTableCell>
          <UiTableCell content="technical">{{ column.defaultValue || '—' }}</UiTableCell>
          <UiTableCell>
            <UiBadge v-if="column.key && column.key !== '—'" tone="info" size="xs">{{ column.key }}</UiBadge>
            <span v-else class="text-text-muted">—</span>
          </UiTableCell>
          <UiTableCell>{{ column.comment }}</UiTableCell>
        </tr>
      </tbody>
    </UiTable>

    <div v-else class="py-[40px] text-center text-caption text-text-muted dark:text-text-muted-dark">
      加载结构中…
    </div>
  </div>
</template>
