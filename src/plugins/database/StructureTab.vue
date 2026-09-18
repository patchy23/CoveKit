<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * 表结构页签：子页签多维信息（列 / 索引 / DDL）
 * 列=字段表格；索引=名称/列/唯一性/类型；DDL=建表语句原文（可复制）。
 * 数据在打开页签时由 openStructureTab 一次性加载（loadColumns + loadStructureExtras）。
 */
import { computed, ref } from 'vue'
import { UiBadge, UiButton, UiIcon, UiIconButton, UiTable, UiTableCell, UiTabs } from '@/core/ui'
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
  const m = db.activeTabId.value.match(/^structure-(?:[^-]+)-(.*)$/)
  return m?.[1] ?? ''
})

function genQuery() {
  db.openSqlEditor()
  db.patchQueryState({ sql: `SELECT * FROM ${tableName.value} LIMIT 100;` })
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <div
      class="flex h-[36px] shrink-0 items-center gap-[8px] border-b border-border px-[12px] dark:border-border-dark"
    >
      <div class="min-w-0">
        <span class="text-card-title font-semibold text-primary dark:text-primary-dark">{{
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
      <div class="ml-auto flex items-center gap-[4px]">
        <UiIconButton
          v-if="subTab === 'ddl' && ddl"
          label="复制 DDL"
          size="xs"
          @click="copyText(ddl)"
        >
          <UiIcon name="copy" :size="12" />
        </UiIconButton>
        <UiButton size="xs" variant="primary" @click="genQuery">生成查询</UiButton>
      </div>
    </div>

    <!-- 列 -->
    <UiScrollArea v-if="subTab === 'columns'" as-child axis="both">
      <div class="min-h-0 flex-1 p-[12px]">
        <UiTable v-if="columns.length" density="compact" :hoverable="true" :striped="true">
          <thead>
            <tr>
              <UiTableCell as="th" resizable>字段</UiTableCell>
              <UiTableCell as="th" resizable>类型</UiTableCell>
              <UiTableCell as="th" resizable>可空</UiTableCell>
              <UiTableCell as="th" resizable>默认值</UiTableCell>
              <UiTableCell as="th" resizable>键</UiTableCell>
              <UiTableCell as="th" resizable>注释</UiTableCell>
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
      <div class="min-h-0 flex-1 p-[12px]">
        <UiTable v-if="indexes.length" density="compact" :hoverable="true" :striped="true">
          <thead>
            <tr>
              <UiTableCell as="th" resizable>索引名</UiTableCell>
              <UiTableCell as="th" resizable>列</UiTableCell>
              <UiTableCell as="th" resizable>唯一</UiTableCell>
              <UiTableCell as="th" resizable>类型/定义</UiTableCell>
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
    <UiScrollArea v-else as-child axis="both">
      <div class="min-h-0 flex-1 p-[12px]">
        <pre
          v-if="ddl"
          class="whitespace-pre-wrap rounded-[8px] border border-border bg-surface-muted p-[12px] font-mono text-body-sm text-primary dark:border-border-dark dark:bg-surface-muted-dark dark:text-primary-dark"
          >{{ ddl }}</pre>
        <div
          v-else
          class="py-[40px] text-center text-caption text-text-muted dark:text-text-muted-dark"
        >
          加载 DDL 中…
        </div>
      </div>
    </UiScrollArea>
  </div>
</template>
