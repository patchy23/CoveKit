<script setup lang="ts">
import { UiIcon, UiIconButton } from '@/core/ui'
import InspectorPanel from './InspectorPanel.vue'
import type { useDatabase } from './useDatabase'

defineProps<{ db: ReturnType<typeof useDatabase>; open: boolean; width: number }>()
const emit = defineEmits<{
  resize: [event: MouseEvent]
  'update:open': [value: boolean]
}>()
</script>

<template>
  <div
    v-if="open"
    class="w-[5px] shrink-0 cursor-col-resize bg-surface-muted transition-colors hover:bg-tertiary/40 dark:bg-surface-muted-dark"
    title="拖拽调整摘要宽度"
    @mousedown="emit('resize', $event)"
  />
  <aside
    v-if="open"
    :style="{ width: `${width}px` }"
    class="flex shrink-0 flex-col border-l border-border dark:border-border-dark"
  >
    <div
      class="flex h-[28px] shrink-0 items-center gap-[4px] border-b border-border px-[8px] dark:border-border-dark"
    >
      <span class="text-caption font-semibold text-primary dark:text-primary-dark">摘要</span>
      <UiIconButton label="收起摘要" size="xs" class="ml-auto" @click="emit('update:open', false)">
        <UiIcon name="chevrons-right" :size="12" />
      </UiIconButton>
    </div>
    <InspectorPanel :db="db" />
  </aside>
  <div
    v-else
    class="flex w-[28px] shrink-0 flex-col items-center gap-[6px] border-l border-border py-[6px] dark:border-border-dark"
  >
    <UiIconButton label="展开摘要" size="xs" @click="emit('update:open', true)">
      <UiIcon name="chevrons-left" :size="12" />
    </UiIconButton>
    <span class="text-caption text-text-muted [writing-mode:vertical-rl] dark:text-text-muted-dark"
      >摘要</span
    >
  </div>
</template>
