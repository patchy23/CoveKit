<script setup lang="ts">
withDefaults(
  defineProps<{ title: string; description?: string; compact?: boolean; selectable?: boolean }>(),
  {
    compact: false,
    description: '',
    selectable: false,
  }
)
</script>

<template>
  <div
    class="flex flex-col items-center justify-center px-lg text-center font-sans"
    :class="[compact ? 'min-h-[64px] py-sm' : 'min-h-[160px] py-xl', { 'select-text': selectable }]"
  >
    <div
      v-if="$slots.icon"
      class="mb-md grid h-11 w-11 place-items-center rounded-lg bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
    >
      <slot name="icon" />
    </div>
    <p
      :class="
        compact
          ? 'text-body-sm font-normal text-secondary dark:text-secondary-dark'
          : 'text-h2 font-semibold text-primary dark:text-primary-dark'
      "
    >
      {{ title }}
    </p>
    <p
      v-if="description"
      class="mt-xs max-w-[440px] text-body-sm text-text-muted dark:text-text-muted-dark"
    >
      {{ description }}
    </p>
    <div v-if="$slots.default" class="mt-md"><slot /></div>
  </div>
</template>
