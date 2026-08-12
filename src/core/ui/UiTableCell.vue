<script setup lang="ts">
import { computed } from 'vue'
import type { UiContentKind } from './types'
import { cn } from './utils'

const props = withDefaults(
  defineProps<{
    as?: 'th' | 'td'
    content?: UiContentKind
    align?: 'left' | 'center' | 'right'
  }>(),
  { as: 'td', content: 'text', align: 'left' }
)

const classes = computed(() =>
  cn('leading-normal', props.as === 'th' ? 'font-sans font-medium' : 'font-normal', {
    'font-sans': ['text', 'numeric', 'status', 'action'].includes(props.content),
    'font-mono': props.content === 'technical' || props.content === 'code',
    'tabular-nums': props.content === 'numeric',
    'text-left': props.align === 'left',
    'text-center': props.align === 'center',
    'text-right': props.align === 'right',
  })
)
</script>

<template>
  <component :is="as" :class="classes" :data-content-kind="content">
    <slot />
  </component>
</template>
