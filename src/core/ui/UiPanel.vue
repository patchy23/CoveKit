<script setup lang="ts">
/**
 * UiPanel · 通用卡片容器
 *
 * collapsible：标题行变成可点的折叠开关（chevron 指示展开态），内容用 v-show 收起，
 * 内部表单控件的已填值与滚动位置在收起/展开间保持；标题行同时支持 Enter 键切换。
 * 非 collapsible 时仍按原样直接渲染默认插槽（不额外包一层 DOM），避免影响既有页面的 flex/grid 布局。
 */
import { computed, ref } from 'vue'
import UiIcon from './UiIcon.vue'

const props = withDefaults(
  defineProps<{
    title?: string
    description?: string
    padding?: 'none' | 'xs' | 'sm' | 'md' | 'lg'
    muted?: boolean
    /** 是否可折叠（默认否，保持既有页面行为不变） */
    collapsible?: boolean
    /** 折叠面板的初始展开态（仅 collapsible 时有意义） */
    defaultOpen?: boolean
  }>(),
  { title: '', description: '', padding: 'md', muted: false, collapsible: false, defaultOpen: true }
)

/** 折叠面板的展开态（非折叠面板恒为展开） */
const open = ref(props.defaultOpen)

/** 是否处于收起态（收起时标题行不留底部间距） */
const collapsed = computed(() => props.collapsible && !open.value)

/** 切换展开态 */
function toggle() {
  open.value = !open.value
}
</script>

<template>
  <section
    class="rounded-lg border border-border dark:border-border-dark"
    :class="[
      muted ? 'bg-surface-muted dark:bg-surface-muted-dark' : 'bg-surface dark:bg-surface-dark',
      padding === 'none'
        ? ''
        : padding === 'xs'
          ? 'p-sm'
          : padding === 'sm'
            ? 'p-[12px]'
            : padding === 'lg'
              ? 'p-lg'
              : 'p-[16px]',
    ]"
  >
    <header
      v-if="title || description || $slots.header || $slots.actions"
      class="flex gap-md"
      :class="collapsed ? '' : 'mb-[12px]'"
    >
      <div
        class="flex min-w-0 flex-1 items-center gap-[6px]"
        :class="collapsible ? 'cursor-pointer select-none' : ''"
        :role="collapsible ? 'button' : undefined"
        :tabindex="collapsible ? 0 : undefined"
        @click="collapsible && toggle()"
        @keydown.enter.prevent="collapsible && toggle()"
      >
        <UiIcon
          v-if="collapsible"
          name="chevron-right"
          :size="14"
          class="shrink-0 text-text-muted transition-transform duration-150 dark:text-text-muted-dark"
          :class="open ? 'rotate-90' : ''"
        />
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
      </div>
      <div v-if="$slots.actions" class="shrink-0"><slot name="actions" /></div>
    </header>
    <div v-if="collapsible" v-show="open"><slot /></div>
    <slot v-else />
  </section>
</template>
