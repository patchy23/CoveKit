<script setup lang="ts">
import { cva } from 'class-variance-authority'
import { computed } from 'vue'
import type { UiSize } from './types'
import { cn } from './utils'
import UiTooltip from './UiTooltip.vue'

defineOptions({ inheritAttrs: false })

const props = withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
    size?: UiSize
    loading?: boolean
    block?: boolean
    as?: 'button' | 'a'
    type?: 'button' | 'submit' | 'reset'
    title?: string
  }>(),
  {
    variant: 'secondary',
    size: 'md',
    loading: false,
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
</script>

<template>
  <UiTooltip :content="title">
    <component
      :is="as"
      v-bind="$attrs"
      :type="as === 'button' ? type : undefined"
      :class="classes"
      :disabled="as === 'button' ? loading || $attrs.disabled === true : undefined"
    >
      <span v-if="loading" class="ui-spinner" aria-hidden="true" />
      <slot />
    </component>
  </UiTooltip>
</template>
