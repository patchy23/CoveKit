<script setup lang="ts">
/**
 * Toast · 全局轻提示（固定深色，视口底部居中；状态在 ui store）
 */
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();
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
.toast-enter-active,
.toast-leave-active {
  transition:
    opacity 0.25s ease,
    transform 0.25s ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, 20px);
}
.toast-enter-to,
.toast-leave-from {
  transform: translate(-50%, 0);
}
</style>
