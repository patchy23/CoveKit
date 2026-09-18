<script setup lang="ts">
import { cva } from 'class-variance-authority'
import { computed } from 'vue'
import { useForwardExpose } from 'reka-ui'
import type { UiSize } from './types'
import { cn } from './utils'
import UiSpinner from './UiSpinner.vue'
import UiTooltip from './UiTooltip.vue'

defineOptions({ inheritAttrs: false })
const { forwardRef } = useForwardExpose()

const props = withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
    size?: UiSize
    loading?: boolean
    disabled?: boolean
    block?: boolean
    as?: 'button' | 'a'
    type?: 'button' | 'submit' | 'reset'
    title?: string
  }>(),
  {
    variant: 'secondary',
    size: 'md',
    loading: false,
    disabled: false,
    block: false,
    as: 'button',
    type: 'button',
    title: '',
  }
)

const buttonVariants = cva('ui-button', {
  variants: {
    variant: {
      primary: 'btn-primary',
      secondary: 'btn-secondary',
      ghost: 'btn-ghost',
      danger: 'btn-danger',
    },
    size: {
      xs: 'ui-control-xs',
      sm: 'ui-control-sm',
      md: 'ui-control-md',
      lg: 'ui-control-lg',
    },
    block: { true: 'w-full justify-center', false: '' },
  },
})

const classes = computed(() =>
  cn(buttonVariants({ variant: props.variant, size: props.size, block: props.block }))
)
const unavailable = computed(() => props.disabled || props.loading)

/** 链接没有原生 disabled，拦截激活并移除导航入口。 */
function guardClick(event: MouseEvent) {
  if (!unavailable.value) return
  event.preventDefault()
  event.stopImmediatePropagation()
}
</script>

<template>
  <UiTooltip :content="title">
    <component
      :is="as"
      :ref="forwardRef"
      v-bind="$attrs"
      :type="as === 'button' ? type : undefined"
      :class="[classes, { 'cursor-not-allowed opacity-60': as === 'a' && unavailable }]"
      :disabled="as === 'button' ? unavailable : undefined"
      :href="as === 'a' && unavailable ? undefined : $attrs.href"
      :tabindex="as === 'a' && unavailable ? -1 : $attrs.tabindex"
      :aria-disabled="unavailable || undefined"
      :aria-busy="loading || undefined"
      @click.capture="guardClick"
    >
      <!-- loading 转圈统一走 UiSpinner（单一实现源；按钮内为纯装饰，aria-hidden 由 UiSpinner 的图形层自带语义、此处整体隐藏避免重复播报） -->
      <span v-if="loading" aria-hidden="true" class="inline-flex">
        <UiSpinner :size="size === 'xs' ? 'xs' : size === 'lg' ? 'md' : 'sm'" label="" />
      </span>
      <slot />
    </component>
  </UiTooltip>
</template>
