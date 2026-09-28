<script setup lang="ts">
import UiScrollArea from './UiScrollArea.vue'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import type { UiContentKind } from './types'
import { useRowWindow } from './collection/useRowWindow'

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
    /** 固定 25px 单行窗口；单元格插槽也须保持单行高度。 */
    virtual?: boolean
    /** 启用接近底部通知，加载中或无后续数据时由消费方关闭。 */
    nearEnd?: boolean
    /** 距底部触发通知的像素距离。 */
    nearEndThreshold?: number
  }>(),
  {
    rowKey: 'id',
    modelValue: '',
    rowNumbers: true,
    height: '260px',
    ariaLabel: '数据结果',
    virtual: false,
    nearEnd: false,
    nearEndThreshold: 250,
  }
)

const emit = defineEmits<{
  (event: 'update:modelValue', value: string): void
  (event: 'cell', payload: { row: Record<string, unknown>; column: UiDataGridColumn }): void
  (event: 'near-end'): void
  (
    event: 'cell-contextmenu',
    payload: { row: Record<string, unknown>; column: UiDataGridColumn },
    mouse: MouseEvent
  ): void
}>()

const root = ref<HTMLElement | null>(null)
const focusedKey = ref('')
const rowHeight = 25
const viewport = useRowWindow(
  computed(() => (props.virtual ? root.value : null)),
  () => props.rows.length,
  () => rowHeight,
  () => rowHeight
)
const focusedIndex = computed(() =>
  focusedKey.value
    ? props.rows.findIndex((row, index) => rowKeyOf(row, index) === focusedKey.value)
    : -1
)
const visibleRows = computed(() => {
  const start = props.virtual ? viewport.start.value : 0
  const end = props.virtual ? viewport.end.value : props.rows.length
  const indexes = Array.from({ length: end - start }, (_, index) => start + index)
  // 原生焦点所在行离屏仍保留；编辑输入框和键盘焦点不能因滚动被卸载。
  if (props.virtual && focusedIndex.value >= 0) {
    if (focusedIndex.value < start) indexes.unshift(focusedIndex.value)
    else if (focusedIndex.value >= end) indexes.push(focusedIndex.value)
  }
  return indexes.map((index, position) => ({
    row: props.rows[index],
    index,
    gap: index - (position ? indexes[position - 1] + 1 : 0),
  }))
})
const trailingRows = computed(() =>
  props.virtual
    ? props.rows.length - ((visibleRows.value[visibleRows.value.length - 1]?.index ?? -1) + 1)
    : 0
)
let endNotified = false
function checkNearEnd() {
  const element = root.value
  if (!props.nearEnd || !element || element.clientHeight <= 0 || !props.rows.length) return
  const near =
    element.scrollHeight - element.scrollTop - element.clientHeight <=
    Math.max(0, props.nearEndThreshold)
  if (!near) endNotified = false
  else if (!endNotified) {
    endNotified = true
    emit('near-end')
  }
}
watch([() => props.rows.length, () => props.nearEnd], () => {
  endNotified = false
  void nextTick(checkNearEnd)
})
watch(
  root,
  (element, _previous, cleanup) => {
    if (!element) return
    const observer =
      typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(checkNearEnd)
    observer?.observe(element)
    window.addEventListener('resize', checkNearEnd)
    void nextTick(checkNearEnd)
    cleanup(() => {
      observer?.disconnect()
      window.removeEventListener('resize', checkNearEnd)
    })
  },
  { flush: 'post' }
)

function ensureRowVisible(index: number) {
  viewport.ensureVisible(index)
  // useRowWindow 的上沿不含 sticky 表头；补齐遮挡范围。
  if (root.value && index * rowHeight < root.value.scrollTop) {
    root.value.scrollTop = index * rowHeight
    root.value.dispatchEvent(new Event('scroll'))
  }
}
function focusRow(index: number) {
  const row = props.rows[index]
  if (!row) return
  focusedKey.value = rowKeyOf(row, index)
  ensureRowVisible(index)
  void nextTick(() =>
    root.value
      ?.querySelector<HTMLElement>(`[data-grid-row="${index}"]`)
      ?.focus({ preventScroll: true })
  )
}
watch(
  [root, () => props.modelValue, () => props.virtual],
  ([, value]) => {
    if (!props.virtual || !value) return
    const index = props.rows.findIndex((row, index) => rowKeyOf(row, index) === value)
    if (index >= 0) ensureRowVisible(index)
  },
  { flush: 'post' }
)
watch(
  () => props.rows,
  () => {
    if (props.virtual && focusedKey.value && focusedIndex.value < 0) {
      focusedKey.value = ''
      if (props.rows.length) focusRow(0)
      else void nextTick(() => root.value?.focus({ preventScroll: true }))
    }
  }
)

function trackFocus(event: FocusEvent) {
  const row =
    event.target instanceof Element ? event.target.closest<HTMLElement>('[data-grid-row]') : null
  const index = Number(row?.dataset.gridRow)
  if (row && props.rows[index]) focusedKey.value = rowKeyOf(props.rows[index], index)
}
function releaseFocus(event: FocusEvent) {
  if (!(event.relatedTarget instanceof Node) || !root.value?.contains(event.relatedTarget))
    focusedKey.value = ''
}

const widths = ref<Record<string, number>>({})
let resizeKey = ''
let resizeStartX = 0
let resizeStartWidth = 0

watch(
  () => props.columns,
  (columns) => {
    // 只持有当前列的宽度；连续查询产生的新别名不能在网格存活期间无限积累。
    const next: Record<string, number> = {}
    for (const column of columns) next[column.key] = widths.value[column.key] ?? column.width ?? 140
    widths.value = next
    if (resizeKey && !columns.some((column) => column.key === resizeKey)) stop()
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
  window.removeEventListener('pointercancel', stop)
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
  window.addEventListener('pointercancel', stop)
  // 拖拽中窗口失焦也要收尾，否则监听器残留
  window.addEventListener('blur', stop)
}

function display(value: unknown) {
  if (value === null) return 'NULL'
  if (value === '') return '(空字符串)'
  const text = String(value ?? '')
  // 虚拟网格只将短预览送入文本节点；插槽与操作事件始终携带原值。
  return props.virtual && text.length > 500 ? text.slice(0, 500) + '…' : text
}

function preview(value: unknown) {
  const text = display(value)
  if (text.length <= 500) return text
  // title 总长不超过 500 个 UTF-16 单元，并避免截断代理对。
  const end = text.charCodeAt(498)
  const length = end >= 0xd800 && end <= 0xdbff ? 498 : 499
  return text.slice(0, length) + '…'
}

/** 行主键（无 rowKey 字段时回退行序号） */
function rowKeyOf(row: Record<string, unknown>, rowIndex: number): string {
  return String(row[props.rowKey] ?? rowIndex)
}

/** 行键盘选中（Enter/空格），与点击同语义 */
function onRowKeydown(event: KeyboardEvent, row: Record<string, unknown>, rowIndex: number) {
  if (event.target !== event.currentTarget) return
  if (props.virtual) {
    const page = Math.max(1, Math.floor((root.value?.clientHeight || 260) / rowHeight) - 1)
    const destinations: Record<string, number> = {
      ArrowUp: rowIndex - 1,
      ArrowDown: rowIndex + 1,
      Home: 0,
      End: props.rows.length - 1,
      PageUp: rowIndex - page,
      PageDown: rowIndex + page,
    }
    if (event.key in destinations) {
      event.preventDefault()
      focusRow(Math.max(0, Math.min(props.rows.length - 1, destinations[event.key])))
      return
    }
  }
  if (event.key !== 'Enter' && event.key !== ' ') return
  event.preventDefault()
  emit('update:modelValue', rowKeyOf(row, rowIndex))
}

function onViewportKeydown(event: KeyboardEvent) {
  if (!props.virtual || event.target !== event.currentTarget) return
  if (['ArrowDown', 'Home', 'End'].includes(event.key)) {
    event.preventDefault()
    focusRow(event.key === 'End' ? props.rows.length - 1 : 0)
  }
}

onBeforeUnmount(stop)
</script>

<template>
  <UiScrollArea as-child axis="both">
    <div
      ref="root"
      class="min-h-0 bg-surface outline-none dark:bg-surface-dark"
      :style="{ height }"
      tabindex="0"
      :aria-label="ariaLabel"
      @scroll.passive="checkNearEnd"
      @keydown="onViewportKeydown"
      @focusin="trackFocus"
      @focusout="releaseFocus"
    >
      <table
        class="border-separate border-spacing-0 text-left text-body-sm"
        :style="{ width: `${tableWidth}px` }"
        :aria-rowcount="virtual ? rows.length + 1 : undefined"
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
          <template
            v-for="{ row, index: rowIndex, gap } in visibleRows"
            :key="rowKeyOf(row, rowIndex)"
          >
            <tr v-if="virtual && gap" aria-hidden="true">
              <td
                :colspan="columns.length + (rowNumbers ? 1 : 0)"
                :style="{ height: `${gap * rowHeight}px`, padding: 0, border: 0 }"
              />
            </tr>
            <tr
              :data-grid-row="rowIndex"
              :aria-rowindex="virtual ? rowIndex + 2 : undefined"
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
              <td
                v-for="column in columns"
                :key="column.key"
                :title="preview(row[column.key])"
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
            </tr>
          </template>
          <tr v-if="trailingRows" aria-hidden="true">
            <td
              :colspan="columns.length + (rowNumbers ? 1 : 0)"
              :style="{ height: `${trailingRows * rowHeight}px`, padding: 0, border: 0 }"
            />
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
