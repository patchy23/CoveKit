<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
    size?: 'sm' | 'md'
    loading?: boolean
    block?: boolean
    as?: 'button' | 'a'
    type?: 'button' | 'submit' | 'reset'
  }>(),
  { variant: 'secondary', size: 'md', loading: false, block: false, as: 'button', type: 'button' }
)

const classes = computed(() => [
  props.variant === 'primary' && 'btn-primary',
  props.variant === 'secondary' && 'btn-secondary',
  props.variant === 'ghost' && 'btn-ghost',
  props.variant === 'danger' && 'btn-danger',
  props.size === 'sm' && 'ui-button-sm',
  props.block && 'w-full justify-center',
])
</script>

<template>
  <component
    :is="as"
    :type="as === 'button' ? type : undefined"
    :class="classes"
    :disabled="as === 'button' ? loading || $attrs.disabled === true : undefined"
  >
    <span v-if="loading" class="ui-spinner" aria-hidden="true" />
    <slot />
  </component>
</template>
