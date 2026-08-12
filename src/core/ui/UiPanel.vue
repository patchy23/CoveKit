<script setup lang="ts">
withDefaults(
  defineProps<{
    title?: string
    description?: string
    padding?: 'none' | 'sm' | 'md'
    muted?: boolean
  }>(),
  { title: '', description: '', padding: 'md', muted: false }
)
</script>

<template>
  <section
    class="rounded-lg border border-border dark:border-border-dark"
    :class="[
      muted ? 'bg-surface-muted dark:bg-surface-muted-dark' : 'bg-surface dark:bg-surface-dark',
      padding === 'none' ? '' : padding === 'sm' ? 'p-[12px]' : 'p-[16px]',
    ]"
  >
    <header
      v-if="title || description || $slots.header || $slots.actions"
      class="mb-[12px] flex gap-md"
    >
      <div class="min-w-0 flex-1">
        <slot name="header">
          <h3 v-if="title" class="text-h2 font-semibold text-primary dark:text-primary-dark">
            {{ title }}
          </h3>
          <p
            v-if="description"
            class="mt-xs text-body-sm text-text-muted dark:text-text-muted-dark"
          >
            {{ description }}
          </p>
        </slot>
      </div>
      <div v-if="$slots.actions" class="shrink-0"><slot name="actions" /></div>
    </header>
    <slot />
  </section>
</template>
