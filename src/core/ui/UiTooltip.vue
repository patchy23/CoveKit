<script setup lang="ts">
/** 统一文字提示：默认插槽只放一个触发元素，as-child 保持原布局与交互语义。 */
import {
  TooltipContent,
  TooltipPortal,
  TooltipProvider,
  TooltipRoot,
  TooltipTrigger,
  useForwardExpose,
} from 'reka-ui'
import { Comment, defineComponent, Fragment, ref, watch, type VNode } from 'vue'
import { useTooltipOwnership } from './useTooltipOwnership'

defineOptions({ inheritAttrs: false })
const props = withDefaults(
  defineProps<{
    content?: string
    side?: 'top' | 'right' | 'bottom' | 'left'
    delayDuration?: number
    disabled?: boolean
  }>(),
  { content: '', side: 'bottom', delayDuration: 400, disabled: false }
)
const open = ref(false)
const { forwardRef } = useForwardExpose()
const ownership = useTooltipOwnership()
function triggerNode(nodes: VNode[]): VNode | undefined {
  for (const node of nodes) {
    if (node.type === Comment) continue
    if (node.type === Fragment) {
      const child = triggerNode(node.children as VNode[])
      if (child) return child
    } else return node
  }
}
/** 隔开 Reka as-child 对插槽 VNode 的改写，保留调用方缓存节点上的原始 ref；不增加 DOM。 */
const TooltipTarget = defineComponent({
  setup(_, { slots }) {
    return () => triggerNode(slots.default?.() ?? [])
  },
})
function close() {
  open.value = false
  ownership.release()
}
/** 仅隔离悬停：按住鼠标的移动继续冒泡，供分隔条和列表拖拽监听。 */
function isolateHover(event: PointerEvent) {
  if (event.buttons !== 0 || event.pointerType === 'touch' || props.disabled || !props.content)
    return
  if (!ownership.isOwner.value) open.value = false
  ownership.claim()
  event.stopPropagation()
}
function focus(event: FocusEvent) {
  if (
    !props.disabled &&
    props.content &&
    event.target instanceof Element &&
    event.target.matches(':focus-visible')
  )
    ownership.claim()
}
function updateOpen(value: boolean) {
  open.value = value && ownership.isOwner.value && !props.disabled && !!props.content
}
watch(
  ownership.isOwner,
  (isOwner) => {
    if (!isOwner) open.value = false
  },
  { flush: 'sync' }
)
watch(
  () => [props.disabled, props.content],
  () => {
    if (props.disabled || !props.content) close()
  }
)
</script>

<template>
  <TooltipProvider
    :delay-duration="delayDuration"
    :skip-delay-duration="0"
    disable-hoverable-content
  >
    <TooltipRoot
      :open="open && ownership.isOwner.value && !disabled && !!content"
      :disabled="disabled || !content"
      ignore-non-keyboard-focus
      disable-hoverable-content
      @update:open="updateOpen"
    >
      <TooltipTrigger
        :ref="forwardRef"
        as-child
        v-bind="$attrs"
        @pointermove="isolateHover"
        @pointerleave="close"
        @pointerdown="close"
        @focus.capture="focus"
        @blur="close"
      >
        <TooltipTarget><slot /></TooltipTarget>
      </TooltipTrigger>
      <TooltipPortal v-if="open && ownership.isOwner.value && !disabled && !!content">
        <TooltipContent
          :aria-label="content"
          :side="side"
          :side-offset="6"
          :collision-padding="8"
          class="ui-tooltip !pointer-events-none z-[240] max-w-[min(280px,var(--reka-tooltip-content-available-width))] select-none whitespace-pre-line break-words rounded-md border border-border bg-surface px-[8px] py-[5px] font-sans text-caption text-secondary shadow-sm dark:border-border-dark dark:bg-surface-dark dark:text-secondary-dark"
        >
          {{ content }}
        </TooltipContent>
      </TooltipPortal>
    </TooltipRoot>
  </TooltipProvider>
</template>
