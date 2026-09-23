<script setup lang="ts">
/** 底部辅助区：隐藏保留实例；宿主为有确定高度的 flex column。 */
import { onUnmounted, ref } from 'vue'
import UiIconButton from './UiIconButton.vue'
import UiIcon from './UiIcon.vue'
import UiToolbar from './UiToolbar.vue'
withDefaults(defineProps<{ open?: boolean; title: string }>(), { open: false })
defineEmits<{ 'update:open': [value: boolean] }>()
const height = ref(260)
let cleanup: (() => void) | undefined
function resize(event: PointerEvent) {
  if (event.button !== 0) return
  event.preventDefault()
  cleanup?.()
  const start = height.value
  const move = (next: PointerEvent) => {
    height.value = Math.min(600, Math.max(120, start + event.clientY - next.clientY))
  }
  cleanup = () => {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', stop)
    window.removeEventListener('pointercancel', stop)
    window.removeEventListener('blur', stop)
  }
  const stop = () => {
    cleanup?.()
    cleanup = undefined
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', stop)
  window.addEventListener('pointercancel', stop)
  window.addEventListener('blur', stop)
}
onUnmounted(() => cleanup?.())
</script>
<template>
  <section
    v-show="open"
    class="flex min-h-0 shrink-0 flex-col border-t border-border bg-surface dark:border-border-dark dark:bg-surface-dark"
    :style="{ height: height + 'px', maxHeight: '60%' }"
  >
    <div
      role="separator"
      aria-label="调整面板高度"
      aria-orientation="horizontal"
      :aria-valuenow="height"
      :aria-valuemin="120"
      :aria-valuemax="600"
      tabindex="0"
      class="h-1 shrink-0 cursor-row-resize touch-none hover:bg-tertiary"
      @pointerdown="resize"
      @keydown.up.prevent="height = Math.min(600, height + 16)"
      @keydown.down.prevent="height = Math.max(120, height - 16)"
    />
    <UiToolbar bordered :title="title"
      ><slot name="actions" /><template #trailing
        ><UiIconButton label="收起面板" size="xs" @click="$emit('update:open', false)"
          ><UiIcon name="chevron-down" :size="14" /></UiIconButton></template
    ></UiToolbar>
    <div class="flex min-h-0 flex-1 flex-col"><slot /></div>
  </section>
</template>
