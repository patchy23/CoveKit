<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * 可视化建表页签（MySQL 系）：表选项 + 列编辑网格 + 实时 DDL 预览 + 执行创建
 * DDL 由 createTableSql.buildCreateTableSql 纯函数生成（标识符引用/注释转义内置）；
 * 执行成功后刷新对象树并打开新表的结构页签。
 */
import { computed, ref } from 'vue'
import {
  UiButton,
  UiCheckbox,
  UiIcon,
  UiIconButton,
  UiInput,
  UiSelect,
  UiTable,
  UiTableCell,
  UiToolbar,
  UiCodeEditor,
} from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { buildCreateTableSql, emptyColumn, type CreateTableColumn } from './createTableSql'
import {
  FALLBACK_CHARSETS,
  MYSQL_COLUMN_TYPES,
  MYSQL_ENGINES,
  MYSQL_TYPES_WITH_LENGTH,
} from './useDatabaseMeta'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props
const { copyText } = useCopy()

const ctx = computed(() => db.activeTabContext.value)

// ── 表选项 ──
const tableName = ref('')
const engine = ref<string>('InnoDB')
const charset = ref<string>('utf8mb4')
const tableComment = ref('')

// ── 列编辑 ──
const columns = ref<CreateTableColumn[]>([
  {
    ...emptyColumn(),
    name: 'id',
    type: 'BIGINT',
    length: '',
    nullable: false,
    autoIncrement: true,
    primary: true,
    comment: '主键',
  },
  emptyColumn(),
])

const typeOptions = MYSQL_COLUMN_TYPES.map((v) => ({ value: v }))
const engineOptions = MYSQL_ENGINES.map((v) => ({ value: v }))
const charsetOptions = FALLBACK_CHARSETS.map((v) => ({ value: v }))

/** INT 系才允许自增 */
function isIntFamily(type: string): boolean {
  return ['INT', 'BIGINT', 'TINYINT', 'SMALLINT'].includes(type.toUpperCase())
}

function onTypeChange(column: CreateTableColumn) {
  if (!MYSQL_TYPES_WITH_LENGTH.has(column.type.toUpperCase())) column.length = ''
  if (!isIntFamily(column.type)) column.autoIncrement = false
}

function addColumn() {
  columns.value.push(emptyColumn())
}

function removeColumn(index: number) {
  columns.value.splice(index, 1)
}

function moveColumn(index: number, delta: -1 | 1) {
  const target = index + delta
  if (target < 0 || target >= columns.value.length) return
  const [row] = columns.value.splice(index, 1)
  columns.value.splice(target, 0, row)
}

/** 实时 DDL 预览 */
const ddl = computed(() =>
  buildCreateTableSql({
    database: ctx.value?.database ?? '',
    table: tableName.value,
    engine: engine.value,
    charset: charset.value,
    comment: tableComment.value,
    columns: columns.value,
  })
)

const creating = ref(false)

async function create() {
  const tabCtx = ctx.value
  if (!tabCtx || !ddl.value || creating.value) return
  creating.value = true
  try {
    const ok = await db.executeDdl(tabCtx.connectionId, ddl.value, tabCtx.database || tabCtx.schema)
    if (ok) {
      // 创建成功：关闭本页签并打开新表结构
      const name = tableName.value.trim()
      db.closeTab(db.activeTabId.value)
      db.openStructureTab(tabCtx.connectionId, name, tabCtx.database, tabCtx.schema)
    }
  } finally {
    creating.value = false
  }
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <!-- 表选项工具行 -->
    <UiToolbar density="compact" bordered class="flex-wrap">
      <span class="shrink-0 text-caption text-secondary dark:text-secondary-dark">表名</span>
      <UiInput v-model="tableName" size="xs" class="w-[140px]" placeholder="new_table" />
      <span class="shrink-0 text-caption text-secondary dark:text-secondary-dark">引擎</span>
      <UiSelect v-model="engine" :options="engineOptions" size="xs" class="w-[100px]" />
      <span class="shrink-0 text-caption text-secondary dark:text-secondary-dark">字符集</span>
      <UiSelect v-model="charset" :options="charsetOptions" size="xs" class="w-[100px]" />
      <UiInput
        v-model="tableComment"
        size="xs"
        class="min-w-0 flex-1"
        placeholder="表注释（可选）"
      />
      <UiButton size="xs" variant="primary" :disabled="!ddl || creating" @click="create">
        {{ creating ? '创建中…' : '创建表' }}
      </UiButton>
    </UiToolbar>

    <!-- 列编辑网格 -->
    <UiScrollArea as-child axis="both">
      <div class="min-h-0 flex-1">
        <UiTable density="compact">
          <thead>
            <tr>
              <UiTableCell as="th" class="w-[28px]"></UiTableCell>
              <UiTableCell as="th">列名</UiTableCell>
              <UiTableCell as="th" class="w-[130px]">类型</UiTableCell>
              <UiTableCell as="th" class="w-[90px]">长度</UiTableCell>
              <UiTableCell as="th" class="w-[48px]">可空</UiTableCell>
              <UiTableCell as="th" class="w-[130px]">默认值</UiTableCell>
              <UiTableCell as="th" class="w-[48px]">自增</UiTableCell>
              <UiTableCell as="th" class="w-[48px]">主键</UiTableCell>
              <UiTableCell as="th">注释</UiTableCell>
              <UiTableCell as="th" class="w-[60px]"></UiTableCell>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(column, index) in columns" :key="index">
              <UiTableCell content="numeric">{{ index + 1 }}</UiTableCell>
              <UiTableCell>
                <UiInput
                  v-model="column.name"
                  size="xs"
                  placeholder="column_name"
                  class="font-mono"
                />
              </UiTableCell>
              <UiTableCell>
                <UiSelect
                  v-model="column.type"
                  :options="typeOptions"
                  size="xs"
                  class="w-full"
                  @update:model-value="onTypeChange(column)"
                />
              </UiTableCell>
              <UiTableCell>
                <UiInput
                  v-model="column.length"
                  size="xs"
                  class="font-mono"
                  :disabled="!MYSQL_TYPES_WITH_LENGTH.has(column.type.toUpperCase())"
                  placeholder="64"
                />
              </UiTableCell>
              <UiTableCell>
                <UiCheckbox v-model="column.nullable" size="xs" />
              </UiTableCell>
              <UiTableCell>
                <UiInput
                  v-model="column.defaultValue"
                  size="xs"
                  class="font-mono"
                  placeholder="空 = 不设"
                />
              </UiTableCell>
              <UiTableCell>
                <UiCheckbox
                  v-model="column.autoIncrement"
                  size="xs"
                  :disabled="!isIntFamily(column.type)"
                />
              </UiTableCell>
              <UiTableCell>
                <UiCheckbox v-model="column.primary" size="xs" />
              </UiTableCell>
              <UiTableCell>
                <UiInput v-model="column.comment" size="xs" placeholder="可选" />
              </UiTableCell>
              <UiTableCell content="action">
                <UiIconButton
                  label="上移"
                  size="xs"
                  :disabled="index === 0"
                  @click="moveColumn(index, -1)"
                >
                  <UiIcon name="chevron-down" :size="12" class="rotate-180" />
                </UiIconButton>
                <UiIconButton label="删除列" size="xs" @click="removeColumn(index)">
                  <UiIcon name="x" :size="12" />
                </UiIconButton>
              </UiTableCell>
            </tr>
          </tbody>
        </UiTable>
        <div class="p-[8px]">
          <UiButton size="xs" variant="secondary" @click="addColumn">+ 添加列</UiButton>
        </div>
      </div>
    </UiScrollArea>

    <!-- DDL 预览 -->
    <div
      class="flex h-[180px] max-h-[40%] min-h-0 shrink-0 flex-col border-t border-border dark:border-border-dark"
    >
      <UiToolbar density="compact" bordered title="DDL 预览">
        <template #trailing
          ><UiIconButton v-if="ddl" label="复制 DDL" size="xs" @click="copyText(ddl)"
            ><UiIcon name="copy" :size="12" /></UiIconButton
        ></template>
      </UiToolbar>
      <UiCodeEditor
        :model-value="ddl || '-- 填写表名与至少一列后生成 DDL'"
        language="sql"
        readonly
        :completion="false"
        class="min-h-0 flex-1 !rounded-none !border-0"
      />
    </div>
  </div>
</template>
