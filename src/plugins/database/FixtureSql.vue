<script setup lang="ts">
import { ref, computed } from 'vue'
import { UiButton, UiModal, UiField, UiInput } from '@/core/ui'
import { fixtureSql } from './fixtureSql'
import type { useDatabase } from './useDatabase'
const props = defineProps<{ db: ReturnType<typeof useDatabase> }>()
const open = ref(false)
const table = ref('test_samples')
const count = ref(100)
const error = ref('')
function show() {
  open.value = true
  error.value = ''
}
const supported = computed(() =>
  ['mysql', 'polardb', 'postgresql', 'sqlite'].includes(
    props.db.activeTabConnection.value?.dbType ?? ''
  )
)
function generate() {
  try {
    const db = props.db
    const context = db.activeTabContext.value
    const sql = fixtureSql(db.activeTabConnection.value?.dbType ?? '', table.value, count.value)
    db.openSqlEditorWithSql(context.connectionId, sql, context.database, context.schema)
    db.patchQueryState({ dirty: true })
    open.value = false
  } catch (cause) {
    error.value = String(cause)
  }
}
</script>
<template>
  <UiButton size="xs" variant="ghost" :disabled="!supported" @click="show()">测试数据模板</UiButton>
  <UiModal
    :open="open"
    title="生成测试数据 SQL"
    description="在当前目标打开新 SQL 草稿；可检查、保存为模板，再决定是否执行。"
    @close="open = false"
  >
    <div class="grid grid-cols-2 gap-sm">
      <UiField label="新表名" size="xs"><UiInput v-model="table" size="xs" /></UiField>
      <UiField label="行数（1–1000）" size="xs"
        ><UiInput v-model.number="count" type="number" size="xs"
      /></UiField>
    </div>
    <p v-if="error" role="alert" class="mt-sm text-caption text-danger">{{ error }}</p>
    <template #footer
      ><UiButton size="xs" variant="primary" @click="generate">生成 SQL 草稿</UiButton></template
    >
  </UiModal>
</template>
