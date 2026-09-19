<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * 表结构页签：子页签多维信息（列 / 索引 / DDL）
 * 列=字段表格；索引=名称/列/唯一性/类型；DDL=建表语句原文（可复制）。
 * 数据在打开页签时由 openStructureTab 一次性加载（loadColumns + loadStructureExtras）。
 */
import { computed, ref } from 'vue'
import {
  UiBadge,
  UiButton,
  UiIcon,
  UiIconButton,
  UiTable,
  UiTableCell,
  UiTabs,
  UiToolbar,
  UiCodeEditor,
} from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props
const { copyText } = useCopy()

/** 子页签：列 / 索引 / DDL */
const subTab = ref<'columns' | 'indexes' | 'ddl'>('columns')
const subTabs = [
  { value: 'columns', label: '列' },
  { value: 'indexes', label: '索引' },
  { value: 'ddl', label: 'DDL' },
]

const columns = computed(() => db.structureColumns.value[db.activeTabId.value] ?? [])
const indexes = computed(() => db.structureIndexes.value[db.activeTabId.value] ?? [])
const ddl = computed(() => db.structureDdl.value[db.activeTabId.value] ?? '')
const tableName = computed(() => {
  return db.activeTabContext.value.table ?? ''
})

function genQuery() {
  const ctx = db.activeTabContext.value
  const mysql = ['mysql', 'polardb'].includes(db.activeTabConnection.value?.dbType ?? '')
  const delimiter = mysql ? '`' : '"'
  const quote = (name: string) =>
    delimiter + name.replaceAll(delimiter, delimiter + delimiter) + delimiter
  const scope = mysql ? ctx.database : ctx.schema
  const qualified = [scope, tableName.value].filter(Boolean).map(quote).join('.')
  db.openSqlEditorWithSql(
    ctx.connectionId,
    `SELECT * FROM ${qualified} LIMIT 100;`,
    ctx.database,
    ctx.schema
  )
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <UiToolbar density="compact" bordered>
      <div class="min-w-0 truncate">
        <span class="text-caption font-semibold text-primary dark:text-primary-dark">{{
          tableName
        }}</span>
        <span class="ml-[8px] font-mono text-caption text-secondary dark:text-secondary-dark">
          {{ db.activeTabConnection.value?.label ?? '' }}
        </span>
      </div>
      <UiTabs
        :model-value="subTab"
        :items="subTabs"
        variant="line"
        size="xs"
        class="ml-[8px]"
        @update:model-value="(v) => (subTab = v as typeof subTab)"
      />
      <template #trailing>
        <UiIconButton
          v-if="subTab === 'ddl' && ddl"
          label="复制 DDL"
          size="xs"
          @click="copyText(ddl)"
        >
          <UiIcon name="copy" :size="12" />
        </UiIconButton>
        <UiButton size="xs" variant="primary" @click="genQuery">生成查询</UiButton>
      </template>
    </UiToolbar>

    <!-- 列 -->
    <UiScrollArea v-if="subTab === 'columns'" as-child axis="both">
      <div class="min-h-0 flex-1 p-[6px]">
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
                <UiBadge v-if="column.key && column.key !== '—'" tone="info" size="xs">{{
                  column.key
                }}</UiBadge>
                <span v-else class="text-text-muted">—</span>
              </UiTableCell>
              <UiTableCell>{{ column.comment }}</UiTableCell>
            </tr>
          </tbody>
        </UiTable>
        <div
          v-else
          class="py-[40px] text-center text-caption text-text-muted dark:text-text-muted-dark"
        >
          加载结构中…
        </div>
      </div>
    </UiScrollArea>

    <!-- 索引 -->
    <UiScrollArea v-else-if="subTab === 'indexes'" as-child axis="both">
      <div class="min-h-0 flex-1 p-[6px]">
        <UiTable v-if="indexes.length" density="compact" :hoverable="true" :striped="true">
          <thead>
            <tr>
              <UiTableCell as="th">索引名</UiTableCell>
              <UiTableCell as="th">列</UiTableCell>
              <UiTableCell as="th">唯一</UiTableCell>
              <UiTableCell as="th">类型/定义</UiTableCell>
            </tr>
          </thead>
          <tbody>
            <tr v-for="index in indexes" :key="index.name">
              <UiTableCell content="technical">{{ index.name }}</UiTableCell>
              <UiTableCell content="technical">{{ index.columns.join(', ') || '—' }}</UiTableCell>
              <UiTableCell>
                <UiBadge :tone="index.nonUnique ? 'neutral' : 'success'" size="xs">
                  {{ index.nonUnique ? '否' : '是' }}
                </UiBadge>
              </UiTableCell>
              <UiTableCell content="technical">{{ index.definition || '—' }}</UiTableCell>
            </tr>
          </tbody>
        </UiTable>
        <div
          v-else
          class="py-[40px] text-center text-caption text-text-muted dark:text-text-muted-dark"
        >
          无索引或该类型暂不支持
        </div>
      </div>
    </UiScrollArea>

    <!-- DDL -->
    <UiCodeEditor
      v-else
      :model-value="ddl || '-- 正在加载 DDL…'"
      language="sql"
      readonly
      :completion="false"
      class="min-h-0 flex-1 !rounded-none !border-0"
    />
  </div>
</template>
