<script setup lang="ts">
/** 公共原生滚动区：as-child 保留原元素；managed 供外部库自行管理视口。 */
import { Primitive, useForwardExpose } from 'reka-ui'

withDefaults(
  defineProps<{
    as?: string
    asChild?: boolean
    axis?: 'vertical' | 'horizontal' | 'both'
    theme?: 'auto' | 'light' | 'dark'
    managed?: boolean
  }>(),
  { as: 'div', asChild: false, axis: 'vertical', theme: 'auto', managed: false }
)
const { forwardRef } = useForwardExpose()
</script>

<template>
  <Primitive
    :ref="forwardRef"
    :as="as"
    :as-child="asChild"
    class="ui-scroll-area"
    :data-scroll-axis="managed ? 'managed' : axis"
    :data-scroll-theme="theme"
  >
    <slot />
  </Primitive>
</template>
