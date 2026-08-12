<script setup lang="ts">
import { computed } from 'vue'
import type { UiSize, UiTone } from './types'

const props = withDefaults(
  defineProps<{ name: string; src?: string; size?: UiSize; tone?: UiTone; square?: boolean }>(),
  { src: '', size: 'md', tone: 'accent', square: false }
)
const initials = computed(() => props.name.trim().slice(0, 2).toUpperCase())
</script>

<template>
  <span
    class="inline-grid shrink-0 place-items-center overflow-hidden font-semibold"
    :class="[
      square ? 'rounded-md' : 'rounded-full',
      size === 'xs'
        ? 'h-[24px] w-[24px] text-caption'
        : size === 'sm'
          ? 'h-[28px] w-[28px] text-caption'
          : size === 'lg'
            ? 'h-[42px] w-[42px] text-body'
            : 'h-[36px] w-[36px] text-body-sm',
      `ui-badge-${tone}`,
    ]"
    :title="name"
  >
    <img v-if="src" :src="src" :alt="name" class="h-full w-full object-cover" />
    <span v-else>{{ initials }}</span>
  </span>
</template>
