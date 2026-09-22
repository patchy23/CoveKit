<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
import UiTooltip from './UiTooltip.vue'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import type { UiContentKind } from './types'

export interface UiDataGridColumn {
  key: string
  label: string
  width?: number
  minWidth?: number
  align?: 'left' | 'right' | 'center'
  content?: UiContentKind
}

const props = withDefaults(
  defineProps<{
    columns: UiDataGridColumn[]
    rows: Record<string, unknown>[]
    rowKey?: string
    modelValue?: string
    rowNumbers?: boolean
    height?: string
    ariaLabel?: string
  }>(),
  {
    rowKey: 'id',
    modelValue: '',
    rowNumbers: true,
    height: '260px',
    ariaLabel: '数据结果',
  }
)

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'cell', payload: { row: Record<string, unknown>; column: UiDataGridColumn }): void
  (
    event: 'cell-contextmenu',
    payload: { row: Record<string, unknown>; column: UiDataGridColumn },
    mouse: MouseEvent
  ): void
}>()

const widths = ref<Record<string, number>>({})
let resizeKey = ''
let resizeStartX = 0
let resizeStartWidth = 0

watch(
  () => props.columns,
  (columns) => {
    const next = { ...widths.value }
    for (const column of columns) next[column.key] ??= column.width ?? 140
    widths.value = next
  },
  { immediate: true, deep: true }
)

const tableWidth = computed(
  () =>
    props.columns.reduce((sum, column) => sum + (widths.value[column.key] ?? 140), 0) +
    (props.rowNumbers ? 42 : 0)
)

function move(event: PointerEvent) {
  const column = props.columns.find((item) => item.key === resizeKey)
  if (!column) return
  widths.value[resizeKey] = Math.max(
    column.minWidth ?? 72,
    resizeStartWidth + event.clientX - resizeStartX
  )
}

function stop() {
  resizeKey = ''
  window.removeEventListener('pointermove', move)
  window.removeEventListener('pointerup', stop)
  window.removeEventListener('blur', stop)
}

function startResize(event: PointerEvent, column: UiDataGridColumn) {
  event.preventDefault()
  event.stopPropagation()
  resizeKey = column.key
  resizeStartX = event.clientX
  resizeStartWidth = widths.value[column.key]
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', stop)
  // 拖拽中窗口失焦也要收尾，否则监听器残留
  window.addEventListener('blur', stop)
}

function display(value: unknown) {
  if (value === null) return 'NULL'
  if (value === '') return '(空字符串)'
  return String(value ?? '')
}

/** 行主键（无 rowKey 字段时回退行序号） */
function rowKeyOf(row: Record<string, unknown>, rowIndex: number): string {
  return String(row[props.rowKey] ?? rowIndex)
}

/** 行键盘选中（Enter/空格），与点击同语义 */
function onRowKeydown(event: KeyboardEvent, row: Record<string, unknown>, rowIndex: number) {
  if (event.target !== event.currentTarget) return
  if (event.key !== 'Enter' && event.key !== ' ') return
  event.preventDefault()
  emit('update:modelValue', rowKeyOf(row, rowIndex))
}

onBeforeUnmount(stop)
</script>

<template>
  <UiScrollArea as-child axis="both">
    <div
      class="min-h-0 bg-surface outline-none dark:bg-surface-dark"
      :style="{ height }"
      tabindex="0"
      :aria-label="ariaLabel"
    >
      <table
        class="border-separate border-spacing-0 text-left text-body-sm"
        :style="{ width: `${tableWidth}px` }"
      >
        <thead class="sticky top-0 z-20 bg-surface-muted dark:bg-surface-muted-dark">
          <tr>
            <th
              v-if="rowNumbers"
              class="sticky left-0 z-30 w-[42px] border-b border-r border-border bg-surface-muted px-[6px] text-center font-sans font-medium text-text-muted dark:border-border-dark dark:bg-surface-muted-dark dark:text-text-muted-dark"
            >
              #
            </th>
            <th
              v-for="column in columns"
              :key="column.key"
              class="group relative h-[25px] border-b border-r border-border px-[7px] font-sans font-medium text-text-muted dark:border-border-dark dark:text-text-muted-dark"
              :style="{ width: `${widths[column.key]}px`, minWidth: `${widths[column.key]}px` }"
            >
              <span class="block truncate">{{ column.label }}</span>
              <span
                class="absolute inset-y-0 right-[-2px] z-10 w-[5px] cursor-col-resize hover:bg-tertiary/60"
                @pointerdown="startResize($event, column)"
              />
            </th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(row, rowIndex) in rows"
            :key="rowKeyOf(row, rowIndex)"
            class="h-[25px] outline-none hover:bg-surface-muted focus-visible:bg-surface-muted dark:hover:bg-surface-muted-dark dark:focus-visible:bg-surface-muted-dark"
            :class="
              modelValue === rowKeyOf(row, rowIndex)
                ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark'
                : ''
            "
            :aria-selected="modelValue === rowKeyOf(row, rowIndex)"
            tabindex="0"
            @click="emit('update:modelValue', rowKeyOf(row, rowIndex))"
            @keydown="onRowKeydown($event, row, rowIndex)"
          >
            <td
              v-if="rowNumbers"
              class="sticky left-0 z-10 border-b border-r border-border bg-surface-muted px-[6px] text-center font-data text-caption tabular-nums text-text-muted dark:border-border-dark dark:bg-surface-muted-dark dark:text-text-muted-dark"
            >
              {{ rowIndex + 1 }}
            </td>
            <UiTooltip
              v-for="column in columns"
              :key="column.key"
              :content="display(row[column.key])"
            >
              <td
                class="max-w-0 truncate border-b border-r border-border px-[7px] text-secondary dark:border-border-dark dark:text-secondary-dark"
                :class="[
                  column.content === 'action' ? 'font-sans' : 'font-data',
                  column.content === 'numeric' ? 'tabular-nums' : '',
                  column.align === 'right'
                    ? 'text-right tabular-nums'
                    : column.align === 'center'
                      ? 'text-center'
                      : 'text-left',
                  row[column.key] === null
                    ? 'italic text-text-muted dark:text-text-muted-dark'
                    : '',
                ]"
                :style="{ width: `${widths[column.key]}px`, minWidth: `${widths[column.key]}px` }"
                @dblclick="emit('cell', { row, column })"
                @contextmenu="emit('cell-contextmenu', { row, column }, $event)"
              >
                <slot
                  :name="`cell-${column.key}`"
                  :row="row"
                  :column="column"
                  :value="row[column.key]"
                >
                  <slot name="cell" :row="row" :column="column" :value="row[column.key]">
                    {{ display(row[column.key]) }}
                  </slot>
                </slot>
              </td>
            </UiTooltip>
          </tr>
          <tr v-if="!rows.length">
            <td
              :colspan="columns.length + (rowNumbers ? 1 : 0)"
              class="px-md py-lg text-center text-body-sm text-text-muted dark:text-text-muted-dark"
            >
              <slot name="empty">暂无数据</slot>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </UiScrollArea>
</template>
