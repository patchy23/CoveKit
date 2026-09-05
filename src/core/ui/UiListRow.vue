<script setup lang="ts">
/**
 * UiListRow · 列表行壳（core/ui 公共组件）
 * 统一侧栏/列表行的尺寸档位、hover/选中态、缩进与光标；内容全部走默认插槽（单行/多行自由布局）。
 * 事件（click/dblclick/contextmenu/pointerdown 等）经属性透传落在行根元素上，调用方直接挂。
 */
withDefaults(
  defineProps<{
    /** 尺寸档位：sm=28px 紧凑（py-5/text-body-sm），md=34px 常规（py-7/text-body） */
    size?: 'sm' | 'md'
    /** 选中态（tertiary-soft 底） */
    active?: boolean
    /** 左侧缩进 px（分组内层级） */
    indent?: number
    /** 光标语义（默认 default） */
    cursor?: 'default' | 'pointer' | 'grab'
  }>(),
  { size: 'md', active: false, indent: 0, cursor: 'default' }
)
</script>

<template>
  <div
    class="mb-[2px] flex items-center rounded-md px-[8px] transition-colors"
    :class="[
      size === 'sm' ? 'py-[5px] text-body-sm' : 'py-[7px] text-body',
      cursor === 'pointer'
        ? 'cursor-pointer'
        : cursor === 'grab'
          ? 'cursor-grab'
          : 'cursor-default',
      active
        ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark'
        : 'hover:bg-border dark:hover:bg-border-dark',
    ]"
    :style="indent ? { marginLeft: `${indent}px` } : undefined"
  >
    <slot />
  </div>
</template>
