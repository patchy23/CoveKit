<script setup lang="ts">
withDefaults(defineProps<{ sticky?: boolean }>(), { sticky: false })
</script>

<template>
  <div class="flex flex-wrap items-center gap-[10px]" :class="{ 'sticky-toolbar': sticky }">
    <slot />
  </div>
</template>
