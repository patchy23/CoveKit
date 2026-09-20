<script setup lang="ts">
import FoundationShowcase from './FoundationShowcase.vue'
import FormShowcase from './FormShowcase.vue'
import SelectionDialogShowcase from './SelectionDialogShowcase.vue'
import DataShowcase from './DataShowcase.vue'
import ExpandableTableShowcase from './ExpandableTableShowcase.vue'
import DatabaseWorkbenchShowcase from './DatabaseWorkbenchShowcase.vue'
import EditorShowcase from './EditorShowcase.vue'
import NavigationShowcase from './NavigationShowcase.vue'
import CollectionShowcase from './CollectionShowcase.vue'
import LogWindowShowcase from './LogWindowShowcase.vue'
import { UiBadge } from '@/core/ui'
</script>

<template>
  <div class="mx-auto flex max-w-[1180px] flex-col gap-md pb-xl">
    <header>
      <div class="flex items-center gap-sm">
        <h1 class="text-h1 font-bold text-primary dark:text-primary-dark">公共组件实验室</h1>
        <UiBadge tone="accent" size="xs">临时工具</UiBadge>
      </div>
      <p class="mt-xs text-body-sm text-secondary dark:text-secondary-dark">
        从紧凑数据页到舒适表单页，统一检查 xs / sm / md / lg 尺寸、状态与常见组合。
      </p>
    </header>

    <FoundationShowcase />
    <FormShowcase />
    <SelectionDialogShowcase />
    <DataShowcase />
    <ExpandableTableShowcase />
    <DatabaseWorkbenchShowcase />
    <EditorShowcase />
    <NavigationShowcase />
    <CollectionShowcase />
    <LogWindowShowcase />
  </div>
</template>
