<script setup lang="ts">
/** 工作区内非模态窗口。父元素须 relative isolate 和确定高度；关闭由宿主卸载实例。 */
import { computed, onMounted, onUnmounted, ref, useId, watch } from 'vue'
import UiIcon from './UiIcon.vue'
import UiIconButton from './UiIconButton.vue'
import { useFloatingWindowOrder } from './floatingWindows'

const props = withDefaults(
  defineProps<{
    title: string
    width?: number
    height?: number
    /** 递增以激活已有窗口，不重建内容。 */
    activation?: number
  }>(),
  { width: 820, height: 520, activation: 0 }
)
defineEmits<{ close: [] }>()
const panel = ref<HTMLElement>()
const titleId = useId()
const { activate, active, index, zIndex } = useFloatingWindowOrder()
const bounds = ref({ width: 1000, height: 700 })
const rect = ref({
  x: 24 + (index % 6) * 28,
  y: 24 + (index % 6) * 28,
  width: props.width,
  height: props.height,
})
const maximized = ref(false)
let observer: ResizeObserver | undefined
let stopGesture: (() => void) | undefined
function constrain() {
  const b = bounds.value
  rect.value.width = Math.min(Math.max(360, rect.value.width), b.width)
  rect.value.height = Math.min(Math.max(240, rect.value.height), b.height)
  rect.value.x = Math.max(0, Math.min(rect.value.x, b.width - rect.value.width))
  rect.value.y = Math.max(0, Math.min(rect.value.y, b.height - rect.value.height))
}
function measure() {
  const parent = panel.value?.parentElement
  if (!parent) return
  // 隐藏工具页不应把用户窗口压成零尺寸。
  if (!parent.clientWidth || !parent.clientHeight) return
  bounds.value = { width: parent.clientWidth, height: parent.clientHeight }
  constrain()
}
const style = computed(() => {
  const r = maximized.value ? { x: 0, y: 0, ...bounds.value } : rect.value
  return {
    left: `${r.x}px`,
    top: `${r.y}px`,
    width: `${r.width}px`,
    height: `${r.height}px`,
    zIndex: zIndex.value,
  }
})
function gesture(event: PointerEvent, resize = false) {
  if (event.button !== 0 || maximized.value) return
  if (!resize && (event.target as HTMLElement).closest('button')) return
  event.preventDefault()
  activate()
  stopGesture?.()
  const start = { ...rect.value }
  const move = (next: PointerEvent) => {
    if (next.pointerId !== event.pointerId) return
    const dx = next.clientX - event.clientX
    const dy = next.clientY - event.clientY
    rect.value = resize
      ? {
          ...start,
          width: Math.min(bounds.value.width - start.x, Math.max(360, start.width + dx)),
          height: Math.min(bounds.value.height - start.y, Math.max(240, start.height + dy)),
        }
      : { ...start, x: start.x + dx, y: start.y + dy }
    constrain()
  }
  const stop = () => {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', stop)
    window.removeEventListener('pointercancel', stop)
    window.removeEventListener('blur', stop)
    stopGesture = undefined
  }
  stopGesture = stop
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', stop)
  window.addEventListener('pointercancel', stop)
  window.addEventListener('blur', stop)
}
function resizeKey(event: KeyboardEvent) {
  const delta = event.shiftKey ? 40 : 10
  if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return
  event.preventDefault()
  rect.value.width += event.key === 'ArrowRight' ? delta : event.key === 'ArrowLeft' ? -delta : 0
  rect.value.height += event.key === 'ArrowDown' ? delta : event.key === 'ArrowUp' ? -delta : 0
  constrain()
}
watch(
  () => props.activation,
  () => {
    activate()
    panel.value?.focus({ preventScroll: true })
  }
)
onMounted(() => {
  measure()
  observer = new ResizeObserver(measure)
  if (panel.value?.parentElement) observer.observe(panel.value.parentElement)
  panel.value?.focus({ preventScroll: true })
})
onUnmounted(() => {
  observer?.disconnect()
  stopGesture?.()
})
</script>

<template>
  <section
    ref="panel"
    role="dialog"
    :aria-labelledby="titleId"
    tabindex="-1"
    class="absolute flex min-h-0 min-w-0 flex-col overflow-hidden rounded-lg border bg-surface text-primary shadow-xl outline-none dark:bg-surface-dark dark:text-primary-dark"
    :class="
      active
        ? 'border-border-strong dark:border-border-strong-dark'
        : 'border-border dark:border-border-dark'
    "
    :style="style"
    @pointerdown.capture="activate"
    @focusin="activate"
  >
    <header
      class="flex shrink-0 touch-none items-center gap-sm border-b border-border bg-surface-muted px-md py-sm select-none dark:border-border-dark dark:bg-surface-muted-dark"
      :class="maximized ? '' : 'cursor-move'"
      @pointerdown="gesture($event)"
      @dblclick.self="maximized = !maximized"
    >
      <h2 :id="titleId" class="min-w-0 flex-1 truncate text-body-sm font-semibold">{{ title }}</h2>
      <UiIconButton
        size="sm"
        :label="maximized ? '还原窗口' : '最大化窗口'"
        @click="maximized = !maximized"
        ><UiIcon :name="maximized ? 'restore' : 'maximize'" :size="14"
      /></UiIconButton>
      <UiIconButton size="sm" label="关闭窗口" @click="$emit('close')"
        ><UiIcon name="x" :size="14"
      /></UiIconButton>
    </header>
    <div class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"><slot /></div>
    <button
      v-if="!maximized"
      type="button"
      aria-label="调整窗口大小，方向键调整"
      class="absolute right-0 bottom-0 h-4 w-4 cursor-se-resize touch-none text-text-muted dark:text-text-muted-dark"
      @pointerdown.stop="gesture($event, true)"
      @keydown="resizeKey"
    >
      <UiIcon name="resize" :size="14" />
    </button>
  </section>
</template>
