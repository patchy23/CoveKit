<script setup lang="ts">
/**
 * BaseModal · 通用弹窗壳
 * 遮罩 + 180ms 上浮淡入 + Esc/遮罩点击关闭（对齐原型 .modal-mask/.modal 与 DESIGN.md 弹窗规范）。
 */
import { onMounted, onUnmounted } from 'vue'

defineProps<{ open: boolean; width?: string }>()
const emit = defineEmits<{ close: [] }>()

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('close')
}
onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="open"
        class="fixed inset-0 z-[180] grid place-items-center bg-[rgba(16,20,28,0.45)] backdrop-blur-[3px]"
      >
        <div
          class="max-h-[85vh] overflow-y-auto rounded-[18px] bg-surface p-[26px] shadow-[0_12px_40px_rgba(16,24,40,0.14)] dark:bg-surface-dark"
          :style="{ width: width ?? 'min(560px, 92vw)' }"
        >
          <slot />
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-enter-active,
.modal-leave-active {
  transition:
    opacity 0.18s ease,
    transform 0.18s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
.modal-enter-from > div,
.modal-leave-to > div {
  transform: translateY(10px) scale(0.98);
}
.modal-enter-active > div,
.modal-leave-active > div {
  transition: transform 0.18s ease;
}
</style>
