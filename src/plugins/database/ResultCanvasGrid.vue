<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { UiDataGridColumn } from '@/core/ui'
import {
  drawGrid,
  gridColumns,
  hitGridCell,
  ROW_HEIGHT,
  HEADER_HEIGHT,
  ROW_NUMBER_WIDTH,
} from './databaseResultCanvas'

type Cell = { row: Record<string, unknown>; column: UiDataGridColumn }
const props = defineProps<{
  columns: UiDataGridColumn[]
  rows: Record<string, unknown>[]
  selected: { row: number; column: number } | null
  editing?: { row: number; column: number } | null
  text: (row: number, column: number) => string
  dirty: (row: number, column: number) => boolean
  revision?: unknown
}>()
const emit = defineEmits<{
  select: [cell: Cell]
  cell: [cell: Cell]
  context: [cell: Cell, event: MouseEvent]
  copy: []
  'editor-offscreen': []
}>()
const root = ref<HTMLElement>()
const canvas = ref<HTMLCanvasElement>()
const width = ref(0)
const height = ref(0)
const scrollLeft = ref(0)
const scrollTop = ref(0)
const overrides = ref<Record<string, number>>({})
const widths = computed(() => props.columns.map((c) => overrides.value[c.key] ?? c.width ?? 140))
const layout = computed(() => gridColumns(widths.value))
const editor = computed(() => {
  if (!props.editing) return null
  const row = props.rows.findIndex((item) => Number(item.__row) === props.editing!.row)
  const column = props.editing.column
  if (row < 0 || !props.columns[column]) return null
  const cellLeft = layout.value.offsets[column]
  const cellTop = HEADER_HEIGHT + row * ROW_HEIGHT
  const cellWidth = widths.value[column]
  const measuredWidth = width.value > 0
  const measuredHeight = height.value > 0
  const left = measuredWidth ? Math.max(cellLeft, scrollLeft.value + ROW_NUMBER_WIDTH) : cellLeft
  const right = measuredWidth
    ? Math.min(cellLeft + cellWidth, scrollLeft.value + width.value)
    : cellLeft + cellWidth
  const top = measuredHeight ? Math.max(cellTop, scrollTop.value + HEADER_HEIGHT) : cellTop
  const bottom = measuredHeight
    ? Math.min(cellTop + ROW_HEIGHT, scrollTop.value + height.value)
    : cellTop + ROW_HEIGHT
  return {
    row,
    cellLeft,
    cellTop,
    cellWidth,
    left,
    top,
    width: Math.max(0, right - left),
    height: Math.max(0, bottom - top),
  }
})
const active = computed(() =>
  props.selected
    ? {
        row: props.rows.findIndex((row) => Number(row.__row) === props.selected!.row),
        column: props.selected.column,
      }
    : undefined
)
const activeText = computed(() => {
  const cell = active.value ?? (props.rows.length ? { row: 0, column: 0 } : undefined)
  if (!cell || cell.row < 0) return ''
  return `${props.columns[cell.column]?.label ?? ''}：${props.text(Number(props.rows[cell.row].__row), cell.column)}`
})
const error = ref('')
let frame = 0
let observer: ResizeObserver | undefined
let themeObserver: MutationObserver | undefined
let drag: { index: number; x: number; width: number } | undefined
let disposed = false
function paint() {
  frame = 0
  const element = root.value
  const surface = canvas.value
  if (!element || !surface || !width.value || !height.value) return
  const ctx = surface.getContext('2d')
  if (!ctx) {
    error.value = '当前环境无法绘制数据表格'
    return
  }
  error.value = ''
  const ratio = Math.min(window.devicePixelRatio || 1, 2)
  const w = Math.ceil(width.value * ratio)
  const h = Math.ceil(height.value * ratio)
  if (surface.width !== w || surface.height !== h) {
    surface.width = w
    surface.height = h
  }
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0)
  const style = getComputedStyle(element)
  ctx.save()
  ctx.translate(0, HEADER_HEIGHT)
  drawGrid(ctx, {
    width: width.value,
    height: Math.max(0, height.value - HEADER_HEIGHT),
    scrollLeft: scrollLeft.value,
    scrollTop: scrollTop.value,
    widths: widths.value,
    rowCount: props.rows.length,
    text: (row, column) => props.text(Number(props.rows[row].__row), column),
    rowLabel: (row) => String(Number(props.rows[row].__row) + 1),
    selected: active.value,
    dirty: (row, column) => props.dirty(Number(props.rows[row].__row), column),
    theme: {
      background: style.getPropertyValue('--grid-background').trim(),
      foreground: style.color,
      muted: style.getPropertyValue('--grid-muted').trim(),
      border: style.getPropertyValue('--grid-border').trim(),
      selected: style.getPropertyValue('--grid-selected').trim(),
      dirty: style.getPropertyValue('--grid-dirty').trim(),
      font: `${style.fontSize} ${style.fontFamily}`,
    },
  })
  ctx.restore()
}
function schedule() {
  if (!disposed && !frame) frame = requestAnimationFrame(paint)
}
function measure() {
  if (!root.value) return
  width.value = root.value.clientWidth
  height.value = root.value.clientHeight
  scrolled()
}
function scrolled() {
  if (!root.value) return
  scrollLeft.value = root.value.scrollLeft
  scrollTop.value = root.value.scrollTop
  schedule()
}
function checkEditorVisibility() {
  const position = editor.value
  if (!props.editing || !root.value || !position) {
    if (props.editing && !position) emit('editor-offscreen')
    return
  }
  if (!height.value || !width.value) return
  if (height.value <= HEADER_HEIGHT || width.value <= ROW_NUMBER_WIDTH) {
    emit('editor-offscreen')
    return
  }

  const top = position.cellTop
  const bottom = top + ROW_HEIGHT
  const left = position.cellLeft
  const right = left + position.cellWidth
  const visibleTop = scrollTop.value + HEADER_HEIGHT
  const visibleBottom = scrollTop.value + height.value
  const visibleLeft = scrollLeft.value + ROW_NUMBER_WIDTH
  const visibleRight = scrollLeft.value + width.value
  if (top < visibleTop || bottom > visibleBottom || right <= visibleLeft || left >= visibleRight)
    emit('editor-offscreen')
}
function cell(row: number, column: number): Cell | undefined {
  if (!props.rows[row] || !props.columns[column]) return
  return { row: props.rows[row], column: props.columns[column] }
}
function point(event: MouseEvent) {
  const rect = root.value!.getBoundingClientRect()
  const hit = hitGridCell(
    event.clientX - rect.left,
    event.clientY - rect.top,
    scrollLeft.value,
    scrollTop.value,
    widths.value,
    props.rows.length
  )
  return hit ? cell(hit.row, Math.max(0, hit.column)) : undefined
}
function hover(event: MouseEvent) {
  const found = point(event)
  if (canvas.value)
    canvas.value.title = found
      ? props.text(Number(found.row.__row), Number(found.column.key.slice(1))).slice(0, 500)
      : ''
}
function click(event: MouseEvent) {
  const found = point(event)
  if (found) {
    root.value?.focus()
    emit('select', found)
  }
}
function open(event: MouseEvent) {
  const found = point(event)
  if (found) emit('cell', found)
}
function context(event: MouseEvent) {
  const found = point(event)
  if (found) {
    event.preventDefault()
    emit('context', found, event)
  }
}
function reveal(row: number, column: number) {
  const element = root.value
  if (!element) return
  const top = row * ROW_HEIGHT
  if (top < element.scrollTop) element.scrollTop = top
  else if (top + ROW_HEIGHT > element.scrollTop + height.value - HEADER_HEIGHT)
    element.scrollTop = top + ROW_HEIGHT - height.value + HEADER_HEIGHT
  const left = layout.value.offsets[column]
  if (left < element.scrollLeft) element.scrollLeft = left
  else if (left + widths.value[column] > element.scrollLeft + width.value)
    element.scrollLeft = left + widths.value[column] - width.value
  scrolled()
}
function keydown(event: KeyboardEvent) {
  if (event.target !== root.value || !props.rows.length || !props.columns.length) return
  let row = Math.max(0, active.value?.row ?? 0)
  let column = active.value?.column ?? 0
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'c') {
    event.preventDefault()
    emit('copy')
    return
  }
  if (event.key === 'Enter' || event.key === 'F2') {
    event.preventDefault()
    emit('cell', cell(row, column)!)
    return
  }
  if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
    event.preventDefault()
    const rect = root.value!.getBoundingClientRect()
    emit(
      'context',
      cell(row, column)!,
      new MouseEvent('contextmenu', { clientX: rect.left + 48, clientY: rect.top + 48 })
    )
    return
  }
  switch (event.key) {
    case 'ArrowUp':
      row--
      break
    case 'ArrowDown':
      row++
      break
    case 'ArrowLeft':
      column--
      break
    case 'ArrowRight':
      column++
      break
    case 'Home':
      column = 0
      if (event.ctrlKey) row = 0
      break
    case 'End':
      column = props.columns.length - 1
      if (event.ctrlKey) row = props.rows.length - 1
      break
    case 'PageUp':
      row -= Math.max(1, Math.floor((height.value - HEADER_HEIGHT) / ROW_HEIGHT))
      break
    case 'PageDown':
      row += Math.max(1, Math.floor((height.value - HEADER_HEIGHT) / ROW_HEIGHT))
      break
    default:
      return
  }
  event.preventDefault()
  row = Math.max(0, Math.min(props.rows.length - 1, row))
  column = Math.max(0, Math.min(props.columns.length - 1, column))
  emit('select', cell(row, column)!)
  reveal(row, column)
}
function resize(index: number, delta: number) {
  const c = props.columns[index]
  overrides.value = {
    ...overrides.value,
    [c.key]: Math.max(c.minWidth ?? 48, Math.min(2000, widths.value[index] + delta)),
  }
}
function move(event: MouseEvent) {
  if (!drag) return
  const c = props.columns[drag.index]
  overrides.value = {
    ...overrides.value,
    [c.key]: Math.max(c.minWidth ?? 48, Math.min(2000, drag.width + event.clientX - drag.x)),
  }
}
function end() {
  drag = undefined
  window.removeEventListener('mousemove', move)
  window.removeEventListener('mouseup', end)
  window.removeEventListener('blur', end)
}
function begin(index: number, event: MouseEvent) {
  event.preventDefault()
  end()
  drag = { index, x: event.clientX, width: widths.value[index] }
  window.addEventListener('mousemove', move)
  window.addEventListener('mouseup', end)
  window.addEventListener('blur', end)
}
watch(
  () => [
    props.editing,
    props.rows,
    props.columns,
    widths.value,
    width.value,
    height.value,
    scrollLeft.value,
    scrollTop.value,
  ],
  checkEditorVisibility,
  { flush: 'post' }
)
watch(
  () => [
    props.rows,
    props.columns,
    props.selected,
    widths.value,
    props.text,
    props.dirty,
    props.revision,
  ],
  schedule
)
watch(
  () => props.columns,
  () => {
    overrides.value = {}
    end()
  }
)
onMounted(() => {
  observer = new ResizeObserver(measure)
  observer.observe(root.value!)
  themeObserver = new MutationObserver(schedule)
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['class', 'style'],
  })
  window.addEventListener('resize', measure)
  void document.fonts?.ready.then(schedule)
  void nextTick(measure)
})
onBeforeUnmount(() => {
  disposed = true
  end()
  observer?.disconnect()
  themeObserver?.disconnect()
  window.removeEventListener('resize', measure)
  cancelAnimationFrame(frame)
})
defineExpose({ focus: () => root.value?.focus() })
</script>

<template>
  <div
    ref="root"
    role="grid"
    tabindex="0"
    aria-label="数据结果"
    :aria-rowcount="rows.length"
    :aria-colcount="columns.length"
    class="result-canvas-grid relative min-h-0 flex-1 overflow-auto text-caption font-mono text-primary dark:text-primary-dark"
    @scroll="scrolled"
    @keydown="keydown"
  >
    <div
      class="relative min-h-full"
      :style="{
        width: `${Math.max(layout.totalWidth, width)}px`,
        height: `${rows.length * ROW_HEIGHT + HEADER_HEIGHT}px`,
      }"
    >
      <canvas
        ref="canvas"
        aria-hidden="true"
        class="sticky top-0 left-0 block"
        :style="{ width: `${width}px`, height: `${height}px` }"
        @click="click"
        @dblclick="open"
        @contextmenu="context"
        @mousemove="hover"
      />
      <div
        role="row"
        class="absolute left-0 flex bg-surface-muted dark:bg-surface-muted-dark border-b border-border dark:border-border-dark"
        :style="{ top: `${scrollTop}px`, height: `${HEADER_HEIGHT}px` }"
      >
        <div
          role="columnheader"
          class="sticky left-0 z-10 shrink-0 bg-surface-muted px-2 dark:bg-surface-muted-dark"
          style="width: 44px"
        >
          #
        </div>
        <div
          v-for="(column, index) in columns"
          :key="column.key"
          role="columnheader"
          class="relative shrink-0 truncate border-r border-border px-2 leading-[25px] dark:border-border-dark"
          :style="{ width: `${widths[index]}px` }"
          :title="column.label"
        >
          {{ column.label }}
          <span
            role="separator"
            tabindex="0"
            aria-orientation="vertical"
            :aria-label="`调整 ${column.label} 列宽`"
            :aria-valuenow="widths[index]"
            class="absolute top-0 right-0 h-full w-2 cursor-col-resize"
            @mousedown.stop="begin(index, $event)"
            @keydown.left.prevent="resize(index, -10)"
            @keydown.right.prevent="resize(index, 10)"
          />
        </div>
      </div>
      <slot
        v-if="editor"
        name="editor"
        :style="{
          position: 'absolute',
          left: `${editor.left}px`,
          top: `${editor.top}px`,
          width: `${editor.width}px`,
          height: `${editor.height}px`,
          zIndex: 20,
        }"
      />
    </div>
    <div v-if="!rows.length" class="absolute top-8 left-2 text-secondary">
      <slot name="empty">暂无数据</slot>
    </div>
    <p v-if="error" role="alert" class="absolute top-8 left-2 text-danger-strong">{{ error }}</p>
    <span role="status" aria-live="polite" class="sr-only">{{ activeText }}</span>
  </div>
</template>

<style scoped>
.result-canvas-grid {
  --grid-background: var(--color-surface);
  --grid-muted: var(--color-text-muted);
  --grid-border: var(--color-border);
  --grid-selected: var(--color-tertiary-soft);
  --grid-dirty: var(--color-warning-soft);
}
:global(.dark) .result-canvas-grid {
  --grid-background: var(--color-surface-dark);
  --grid-muted: var(--color-text-muted-dark);
  --grid-border: var(--color-border-dark);
  --grid-selected: var(--color-tertiary-soft-dark);
  --grid-dirty: var(--color-warning-soft-dark);
}
</style>
