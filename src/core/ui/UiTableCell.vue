<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import type { UiContentKind } from './types'
import { cn } from './utils'

const props = withDefaults(
  defineProps<{
    as?: 'th' | 'td'
    content?: UiContentKind
    align?: 'left' | 'center' | 'right'
    /** 表头拖拽调宽（默认开启，仅 as="th" 生效）：右缘拖拽手柄，双击复位自动宽度 */
    resizable?: boolean
  }>(),
  { as: 'td', content: 'text', align: 'left', resizable: true }
)

const classes = computed(() =>
  cn('leading-normal', props.as === 'th' ? 'font-sans font-medium' : 'font-normal', {
    // 表头文字不换行：换行会撑破行高、整表错位；宽度不足时由外层横向滚动兜底
    'whitespace-nowrap': props.as === 'th',
    // 拖拽手柄按右缘绝对定位（table-cell 支持 relative）
    relative: props.as === 'th' && props.resizable,
    'font-data': props.as === 'td' && props.content !== 'action',
    'font-sans': props.as === 'td' && props.content === 'action',
    'tabular-nums': props.content === 'numeric',
    'text-left': props.align === 'left',
    'text-center': props.align === 'center',
    'text-right': props.align === 'right',
  })
)

/** 单元格根元素（:is 是原生标签，ref 拿到 DOM）；列宽状态直接写在 DOM style 上 */
const cellRef = ref<HTMLElement | null>(null)
/** 拖拽起点：指针 X 与该时刻的列宽 */
let dragStartX = 0
let dragStartWidth = 0

/** 列宽下限（再小会被内容 min-content 自然顶住，这里只是兜住 0/负值） */
const MIN_COLUMN_WIDTH = 40
let stopResize: (() => void) | undefined

onBeforeUnmount(() => stopResize?.())
watch(
  () => [props.as, props.resizable],
  () => stopResize?.()
)

function startResize(event: PointerEvent): void {
  const cell = cellRef.value
  if (!cell || event.button !== 0) return
  stopResize?.()
  event.preventDefault()
  dragStartX = event.clientX
  dragStartWidth = cell.getBoundingClientRect().width
  // 拖拽期间禁止选中文字（松手恢复）
  const alreadyLocked = document.body.classList.contains('ui-drag-select-lock')
  document.body.classList.add('ui-drag-select-lock')
  const onMove = (move: PointerEvent) => {
    const next = Math.max(MIN_COLUMN_WIDTH, Math.round(dragStartWidth + move.clientX - dragStartX))
    // width + minWidth 同写：table auto 布局下 width 只是建议值，minWidth 防止其他列把它压回去
    cell.style.width = `${next}px`
    cell.style.minWidth = `${next}px`
  }
  const onUp = () => {
    if (!alreadyLocked) document.body.classList.remove('ui-drag-select-lock')
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onUp)
    window.removeEventListener('pointercancel', onUp)
    window.removeEventListener('blur', onUp)
    stopResize = undefined
  }
  stopResize = onUp
  window.addEventListener('pointermove', onMove)
  window.addEventListener('pointerup', onUp)
  window.addEventListener('pointercancel', onUp)
  window.addEventListener('blur', onUp)
}

/** 双击手柄复位：清掉行内宽度，回到自动布局 */
function resetWidth(): void {
  const cell = cellRef.value
  if (!cell) return
  cell.style.width = ''
  cell.style.minWidth = ''
}
</script>

<template>
  <component :is="as" ref="cellRef" :class="classes" :data-content-kind="content">
    <slot />
    <span
      v-if="as === 'th' && resizable"
      aria-hidden="true"
      class="absolute inset-y-0 right-[-2px] z-10 w-[5px] cursor-col-resize hover:bg-tertiary/60"
      @pointerdown.stop="startResize"
      @dblclick.stop="resetWidth"
    />
  </component>
</template>
