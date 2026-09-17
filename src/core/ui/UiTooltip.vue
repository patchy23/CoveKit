<script setup lang="ts">
/** 统一文字提示：默认插槽只放一个触发元素，as-child 保持原布局与交互语义。 */
import {
  Primitive,
  TooltipContent,
  TooltipPortal,
  TooltipProvider,
  TooltipRoot,
  TooltipTrigger,
} from 'reka-ui'
import { ref, watch } from 'vue'

defineOptions({ inheritAttrs: false })
const props = withDefaults(
  defineProps<{
    content?: string
    side?: 'top' | 'right' | 'bottom' | 'left'
    delayDuration?: number
    disabled?: boolean
  }>(),
  { content: '', side: 'top', delayDuration: 400, disabled: false }
)
const open = ref(false)
watch(
  () => [props.disabled, props.content],
  () => {
    if (props.disabled || !props.content) open.value = false
  }
)
</script>

<template>
  <TooltipProvider v-if="content" :delay-duration="delayDuration">
    <TooltipRoot
      :open="open && !disabled"
      :disabled="disabled"
      ignore-non-keyboard-focus
      @update:open="open = !disabled && $event"
    >
      <TooltipTrigger as-child v-bind="$attrs">
        <slot />
      </TooltipTrigger>
      <TooltipPortal>
        <TooltipContent
          :aria-label="content"
          :side="side"
          :side-offset="6"
          :collision-padding="8"
          class="ui-tooltip pointer-events-auto z-[240] max-w-[min(280px,var(--reka-tooltip-content-available-width))] whitespace-pre-line break-words rounded-md border border-border bg-surface px-[8px] py-[5px] font-sans text-caption text-secondary shadow-sm dark:border-border-dark dark:bg-surface-dark dark:text-secondary-dark"
        >
          {{ content }}
        </TooltipContent>
      </TooltipPortal>
    </TooltipRoot>
  </TooltipProvider>
  <Primitive v-else as-child v-bind="$attrs">
    <slot />
  </Primitive>
</template>
