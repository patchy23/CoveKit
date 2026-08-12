<script setup lang="ts">
defineProps<{ title: string; description?: string }>()
</script>

<template>
  <div class="flex min-h-[160px] flex-col items-center justify-center px-lg py-xl text-center">
    <div
      v-if="$slots.icon"
      class="mb-md grid h-11 w-11 place-items-center rounded-lg bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
    >
      <slot name="icon" />
    </div>
    <p class="text-h2 font-semibold text-primary dark:text-primary-dark">{{ title }}</p>
    <p
      v-if="description"
      class="mt-xs max-w-[440px] text-body-sm text-text-muted dark:text-text-muted-dark"
    >
      {{ description }}
    </p>
    <div v-if="$slots.default" class="mt-md"><slot /></div>
  </div>
</template>
