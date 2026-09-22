<script setup lang="ts">
/** 可停留的非模态详情；悬停不抢焦点，点击保持，Esc 返回触发按钮。 */
import { nextTick, onBeforeUnmount, ref, useId } from 'vue'
import { PopoverRoot, PopoverAnchor, PopoverPortal, PopoverContent } from 'reka-ui'
import UiButton from './UiButton.vue'
import { UI_FLOATING_PANEL_CLASS } from './utils'

defineProps<{ label: string }>()
const open = ref(false)
const pinned = ref(false)
const anchor = ref<HTMLElement>()
const contentId = useId()
let timer: ReturnType<typeof setTimeout> | undefined
let focusInside = false
function cancelTimer() {
  clearTimeout(timer)
}
function enter(event: PointerEvent) {
  cancelTimer()
  if (event.pointerType === 'touch' || event.buttons) return
  timer = setTimeout(() => {
    open.value = true
  }, 400)
}
function leave() {
  cancelTimer()
  if (pinned.value || focusInside) return
  timer = setTimeout(close, 180)
}
function close() {
  cancelTimer()
  open.value = false
  pinned.value = false
  focusInside = false
}
function focusIn() {
  focusInside = true
  cancelTimer()
}
function focusOut(event: FocusEvent) {
  if (
    event.relatedTarget instanceof Node &&
    event.currentTarget instanceof Node &&
    event.currentTarget.contains(event.relatedTarget)
  )
    return
  focusInside = false
  leave()
}
async function toggle(event: MouseEvent) {
  cancelTimer()
  if (pinned.value) close()
  else {
    pinned.value = true
    open.value = true
    if (event.detail === 0) {
      await nextTick()
      document.getElementById(contentId)?.focus()
    }
  }
}
function outside(event: CustomEvent<{ originalEvent: Event }>) {
  // Anchor 不是 Reka Trigger；拦住触发区 pointerdown，交给随后 click 切换保持状态。
  const target = event.detail.originalEvent.target
  if (target instanceof Node && anchor.value?.contains(target)) event.preventDefault()
}
async function escape() {
  close()
  await nextTick()
  anchor.value?.querySelector('button')?.focus()
}
onBeforeUnmount(cancelTimer)
defineExpose({ close })
</script>

<template>
  <PopoverRoot :open="open" :modal="false" @update:open="!$event && close()">
    <PopoverAnchor as-child>
      <span ref="anchor" class="block" @pointerenter="enter" @pointerleave="leave">
        <UiButton
          variant="ghost"
          block
          class="!h-auto !justify-start !px-[10px] !py-sm"
          :aria-label="label"
          aria-haspopup="dialog"
          :aria-expanded="open"
          :aria-controls="open ? contentId : undefined"
          @click="toggle"
        >
          <slot name="trigger" />
        </UiButton>
      </span>
    </PopoverAnchor>
    <PopoverPortal>
      <PopoverContent
        :id="contentId"
        tabindex="-1"
        side="right"
        align="end"
        :side-offset="8"
        :collision-padding="8"
        :aria-label="label"
        :class="UI_FLOATING_PANEL_CLASS"
        class="w-[320px] max-w-[calc(100vw-16px)] p-md"
        @pointerenter="cancelTimer"
        @pointerleave="leave"
        @focusin="focusIn"
        @focusout="focusOut"
        @open-auto-focus.prevent
        @close-auto-focus.prevent
        @escape-key-down="escape"
        @interact-outside="outside"
      >
        <slot :close="close" :pinned="pinned" />
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>
