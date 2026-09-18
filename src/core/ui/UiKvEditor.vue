<script setup lang="ts">
/**
 * UiKvEditor · 键值对行编辑器（HTTP Params/Headers、连接参数、环境变量等场景共用）
 * 形态：表头（键/值/操作列）+ 行（两个 UiInput + 删除）+「+ 添加」按钮；行 id 由组件生成。
 */
import UiButton from './UiButton.vue'
import UiIcon from './UiIcon.vue'
import UiIconButton from './UiIconButton.vue'
import UiInput from './UiInput.vue'

export interface UiKvRow {
  id: string
  key: string
  value: string
}

const props = withDefaults(
  defineProps<{
    rows: UiKvRow[]
    /** 表头与占位文案（缺省为通用「名/值」） */
    keyLabel?: string
    valueLabel?: string
    keyPlaceholder?: string
    valuePlaceholder?: string
    addLabel?: string
    removeLabel?: string
  }>(),
  {
    keyLabel: '名',
    valueLabel: '值',
    keyPlaceholder: 'key',
    valuePlaceholder: 'value',
    addLabel: '添加一行',
    removeLabel: '删除该行',
  }
)

const emit = defineEmits<{
  (e: 'update:rows', rows: UiKvRow[]): void
}>()

function addRow(): void {
  emit('update:rows', [...props.rows, { id: crypto.randomUUID(), key: '', value: '' }])
}

function removeRow(id: string): void {
  emit(
    'update:rows',
    props.rows.filter((row) => row.id !== id)
  )
}

function setRow(id: string, field: 'key' | 'value', value: string): void {
  emit(
    'update:rows',
    props.rows.map((row) => (row.id === id ? { ...row, [field]: value } : row))
  )
}
</script>

<template>
  <div>
    <div
      class="mb-[6px] grid grid-cols-[1fr_1fr_36px] gap-[8px] px-[2px] text-caption font-medium text-text-muted dark:text-text-muted-dark"
    >
      <span>{{ keyLabel }}</span>
      <span>{{ valueLabel }}</span>
      <span />
    </div>
    <div v-for="row in rows" :key="row.id" class="mb-[6px] grid grid-cols-[1fr_1fr_36px] gap-[8px]">
      <UiInput
        :value="row.key"
        size="sm"
        class="font-data"
        :placeholder="keyPlaceholder"
        spellcheck="false"
        @update:model-value="setRow(row.id, 'key', String($event))"
      />
      <UiInput
        :value="row.value"
        size="sm"
        class="font-data"
        :placeholder="valuePlaceholder"
        spellcheck="false"
        @update:model-value="setRow(row.id, 'value', String($event))"
      />
      <UiIconButton :label="removeLabel" size="sm" @click="removeRow(row.id)">
        <UiIcon name="trash" :size="13" />
      </UiIconButton>
    </div>
    <UiButton variant="ghost" size="sm" @click="addRow">+ {{ addLabel }}</UiButton>
  </div>
</template>
