<script setup lang="ts">
/**
 * UiToolbar · 工具栏：纯操作行（默认）或页面/区块头部栏（bordered + title）。
 * 头部栏形态：flex shrink-0 items-center gap-[10px] border-b px-[12px] py-[8px]，标题在左。
 * 右侧操作一律放 #trailing（自动 ml-auto），不要在默认插槽里手写 ml-auto。
 * sticky 形态由 .sticky-toolbar 完整定义（含边框与内边距），不与 bordered 叠加。
 */
withDefaults(
  defineProps<{
    /** 紧凑工作台档位；默认保持现有布局。 */
    density?: 'default' | 'compact'
    /** 栏目标题（左侧，text-body-sm text-secondary） */
    title?: string
    sticky?: boolean
    /** 头部栏形态：下边框 + 固定内边距（默认是纯操作行，不加边框） */
    bordered?: boolean
  }>(),
  { title: '', sticky: false, bordered: false, density: 'default' }
)
</script>

<template>
  <div
    class="flex shrink-0 items-center"
    :class="[
      density === 'compact' ? 'min-h-[28px] gap-[4px]' : 'flex-wrap gap-[10px]',
      sticky
        ? 'sticky-toolbar'
        : [
            bordered
              ? [
                  'border-b border-border dark:border-border-dark',
                  density === 'compact' ? 'px-[6px] py-[2px]' : 'px-[12px] py-[8px]',
                ]
              : '',
          ],
    ]"
  >
    <span v-if="title" class="text-body-sm text-secondary dark:text-secondary-dark">{{
      title
    }}</span>
    <slot />
    <div
      v-if="$slots.trailing"
      class="ml-auto flex items-center"
      :class="density === 'compact' ? 'shrink-0 gap-[4px]' : 'gap-[10px]'"
    >
      <slot name="trailing" />
    </div>
  </div>
</template>
