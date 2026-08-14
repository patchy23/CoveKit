<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'

const props = withDefaults(
  defineProps<{
    modelValue: number
    direction?: 'horizontal' | 'vertical'
    min?: number
    max?: number
    label?: string
  }>(),
  { direction: 'horizontal', min: 140, max: 720, label: '调整分栏大小' }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: number): void }>()
const root = ref<HTMLElement | null>(null)
const dragging = ref(false)

function clampPrimary(value: number): number {
  const lower = Math.min(props.min, props.max)
  const upper = Math.max(props.min, props.max)
  const safe = Number.isFinite(value) ? value : lower
  return Math.min(Math.max(safe, lower), upper)
}

const primarySize = computed(() => clampPrimary(props.modelValue))
const template = computed(() => `${primarySize.value}px 4px minmax(0, 1fr)`)

function move(event: PointerEvent) {
  const bounds = root.value?.getBoundingClientRect()
  if (!bounds) return
  const raw =
    props.direction === 'horizontal' ? event.clientX - bounds.left : event.clientY - bounds.top
  emit('update:modelValue', clampPrimary(Math.round(raw)))
}

function stop() {
  dragging.value = false
  window.removeEventListener('pointermove', move)
  window.removeEventListener('pointerup', stop)
  window.removeEventListener('blur', stop)
}

function start(event: PointerEvent) {
  event.preventDefault()
  dragging.value = true
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', stop)
  window.addEventListener('blur', stop)
}

function onKeydown(event: KeyboardEvent) {
  const horizontal = props.direction === 'horizontal'
  const forward = horizontal ? event.key === 'ArrowRight' : event.key === 'ArrowDown'
  const backward = horizontal ? event.key === 'ArrowLeft' : event.key === 'ArrowUp'
  if (forward || backward) {
    event.preventDefault()
    emit('update:modelValue', clampPrimary(primarySize.value + (forward ? 16 : -16)))
  } else if (event.key === 'Home' || event.key === 'End') {
    event.preventDefault()
    emit('update:modelValue', clampPrimary(event.key === 'Home' ? props.min : props.max))
  }
}

onBeforeUnmount(() => {
  stop()
})
</script>

<template>
  <div
    ref="root"
    class="grid min-h-0 min-w-0 overflow-hidden"
    :class="[
      direction === 'horizontal' ? 'grid-flow-col' : 'grid-flow-row',
      { 'select-none': dragging },
    ]"
    :style="
      direction === 'horizontal'
        ? { gridTemplateColumns: template }
        : { gridTemplateRows: template }
    "
  >
    <div class="min-h-0 min-w-0 overflow-hidden"><slot name="primary" /></div>
    <div
      role="separator"
      :aria-orientation="direction"
      :aria-label="label"
      :aria-valuemin="Math.min(min, max)"
      :aria-valuemax="Math.max(min, max)"
      :aria-valuenow="primarySize"
      tabindex="0"
      class="group relative z-10 touch-none bg-border/70 transition-colors hover:bg-tertiary focus-visible:bg-tertiary focus-visible:outline-none dark:bg-border-dark/70 dark:hover:bg-tertiary-dark dark:focus-visible:bg-tertiary-dark"
      :class="direction === 'horizontal' ? 'cursor-col-resize' : 'cursor-row-resize'"
      @pointerdown="start"
      @keydown="onKeydown"
    >
      <span
        class="absolute rounded-full bg-border-strong opacity-0 transition-opacity group-hover:opacity-100 dark:bg-border-strong-dark"
        :class="
          direction === 'horizontal'
            ? 'inset-y-[40%] left-[1px] w-[2px]'
            : 'inset-x-[46%] top-[1px] h-[2px]'
        "
      />
    </div>
    <div class="min-h-0 min-w-0 overflow-hidden"><slot name="secondary" /></div>
  </div>
</template>
