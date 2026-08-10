<script setup lang="ts">
/**
 * Toast · 全局轻提示（固定深色，视口底部居中；状态在 ui store）
 */
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
</script>

<template>
  <Teleport to="body">
    <Transition name="toast">
      <div
        v-if="ui.toastVisible"
        class="fixed bottom-[34px] left-1/2 z-[200] -translate-x-1/2 rounded-md bg-[#1c2129] px-[18px] py-[10px] text-body text-white shadow-[0_12px_40px_rgba(16,24,40,0.14)]"
      >
        {{ ui.toastMessage }}
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* 水平居中由容器的原生 translate 负责（Tailwind 4 translate 属性），
   动画只做垂直位移与透明度，避免 transform 叠加导致先靠左。 */
.toast-enter-active,
.toast-leave-active {
  transition:
    opacity 0.25s ease,
    transform 0.25s ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(20px);
}
.toast-enter-to,
.toast-leave-from {
  transform: translateY(0);
}
</style>
