<script setup lang="ts">
/**
 * Toast · 全局轻提示与可复制详情（主题跟随应用；计时状态在 ui store）
 */
import { useUiStore } from '@/stores/ui'
import { onUnmounted, ref } from 'vue'
import { UiButton, UiModal, UiScrollArea } from '@/core/ui'
import { writeClipboardText } from '@/core/platform/clipboard'

const ui = useUiStore()
const detail = ref<string | null>(null)
const copyStatus = ref('')
let hovered = false
let focused = false
function interaction(kind: 'hover' | 'focus', active: boolean) {
  if (kind === 'hover') hovered = active
  else focused = active
  if (hovered || focused) ui.pauseToast()
  else ui.resumeToast()
}
function openDetails() {
  detail.value = ui.toastMessage
  copyStatus.value = ''
  hovered = focused = false
  ui.dismissToast()
}
function focusOut(event: FocusEvent) {
  if (
    !(event.relatedTarget instanceof Node) ||
    !(event.currentTarget as HTMLElement).contains(event.relatedTarget)
  ) {
    interaction('focus', false)
  }
}
async function copyDetail() {
  const text = detail.value
  if (text === null) return
  const result = await writeClipboardText(text)
  if (detail.value === text)
    copyStatus.value = result.ok ? '已复制' : '复制失败，请选择正文手动复制'
}
onUnmounted(() => ui.resumeToast())
</script>

<template>
  <Teleport to="body">
    <Transition name="toast">
      <div
        v-if="ui.toastVisible"
        class="pointer-events-auto fixed bottom-[34px] left-1/2 z-[200] flex max-w-[calc(100vw-32px)] -translate-x-1/2 items-center gap-sm rounded-md border border-border bg-surface px-md py-sm text-body text-primary shadow-lg dark:border-border-dark dark:bg-surface-dark dark:text-primary-dark"
        @mouseenter="interaction('hover', true)"
        @mouseleave="interaction('hover', false)"
        @focusin="interaction('focus', true)"
        @focusout="focusOut"
      >
        <span role="status" class="select-text line-clamp-3 min-w-0 break-all">{{
          ui.toastMessage
        }}</span>
        <UiButton size="xs" variant="ghost" class="shrink-0" @click="openDetails">详情</UiButton>
      </div>
    </Transition>
  </Teleport>
  <UiModal :open="detail !== null" title="消息详情" @close="detail = null">
    <UiScrollArea class="max-h-[50vh]" axis="both">
      <p class="select-text whitespace-pre-wrap break-all text-body-sm">{{ detail }}</p>
    </UiScrollArea>
    <template #footer>
      <span
        role="status"
        class="select-text text-caption text-secondary dark:text-secondary-dark"
        >{{ copyStatus }}</span
      >
      <UiButton size="sm" @click="copyDetail">复制内容</UiButton>
      <UiButton size="sm" variant="ghost" @click="detail = null">关闭</UiButton>
    </template>
  </UiModal>
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
