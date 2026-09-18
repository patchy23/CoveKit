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
    // 表头文字不换行：换行会撑破行高、整表错位；宽度不足时由外层横向滚动兜底
    'whitespace-nowrap': props.as === 'th',
    'font-data': props.as === 'td' && props.content !== 'action',
    'font-sans': props.as === 'td' && props.content === 'action',
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
