<script setup lang="ts">
import { SwitchRoot, SwitchThumb } from 'reka-ui'
import { computed } from 'vue'
import UiTooltip from './UiTooltip.vue'
import type { UiSize } from './types'

const props = withDefaults(
  defineProps<{
    modelValue: boolean
    label?: string
    title?: string
    description?: string
    disabled?: boolean
    size?: UiSize
  }>(),
  { label: '', title: '', description: '', disabled: false, size: 'md' }
)

const emit = defineEmits<{ (event: 'update:modelValue', value: boolean): void }>()
const trackClass = computed(
  () =>
    ({
      xs: 'h-[16px] w-[28px] p-[2px]',
      sm: 'h-[18px] w-[32px] p-[2px]',
      md: 'h-[22px] w-[38px] p-[2px]',
      lg: 'h-[26px] w-[46px] p-[3px]',
    })[props.size]
)
const thumbClass = computed(
  () =>
    ({
      xs: 'h-[12px] w-[12px] translate-x-[12px]',
      sm: 'h-[14px] w-[14px] translate-x-[14px]',
      md: 'h-[18px] w-[18px] translate-x-[16px]',
      lg: 'h-[20px] w-[20px] translate-x-[20px]',
    })[props.size]
)
</script>

<template>
  <label
    class="inline-flex items-center gap-sm"
    :class="disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'"
  >
    <UiTooltip :content="title">
      <SwitchRoot
        :aria-label="label || title || undefined"
        class="shrink-0 rounded-full transition-colors"
        :class="[
          trackClass,
          modelValue
            ? 'bg-tertiary-strong dark:bg-tertiary-dark'
            : 'bg-border-strong dark:bg-border-strong-dark',
        ]"
        :model-value="modelValue"
        :disabled="disabled"
        @update:model-value="emit('update:modelValue', $event)"
      >
        <SwitchThumb
          class="block rounded-full bg-surface shadow-sm transition-transform dark:bg-primary-dark"
          :class="[thumbClass, { '!translate-x-0': !modelValue }]"
        />
      </SwitchRoot>
    </UiTooltip>
    <span v-if="label || description">
      <span v-if="label" class="block text-body font-medium text-primary dark:text-primary-dark">{{
        label
      }}</span>
      <span
        v-if="description"
        class="block text-caption text-text-muted dark:text-text-muted-dark"
        >{{ description }}</span
      >
    </span>
  </label>
</template>
