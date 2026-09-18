<script setup lang="ts">
/**
 * UiTabsOverflow · 页签溢出收纳（UiDropdownMenu 的页签特化薄封装）
 * 触发器「···」+ 下拉列出溢出页签；键盘导航/Esc/焦点归还/锚定由 UiDropdownMenu(reka)承载。
 * 用法：父组件按宽度把放不下的页签传给 items；选中 emit('select')，行内关闭 emit('close')。
 */
import { computed } from 'vue'
import UiDropdownMenu from './UiDropdownMenu.vue'
import UiIcon from './UiIcon.vue'
import UiTabStatusDot from './UiTabStatusDot.vue'
import type { UiTabItem } from './UiTabs.vue'

const props = defineProps<{
  items: UiTabItem[]
  /** 当前激活页签（用于高亮标记） */
  modelValue?: string
}>()

const emit = defineEmits<{
  (event: 'select', value: string): void
  (event: 'close', value: string): void
}>()

/** 映射为菜单项（value 即页签 value；页签没有禁用语义，不设 disabled） */
const menuItems = computed(() =>
  props.items.map((item) => ({ value: item.value, label: item.label }))
)
/** value → 原始页签项（状态点/可关闭标记按它取） */
const itemByValue = computed(() => new Map(props.items.map((item) => [item.value, item])))
</script>

<template>
  <UiDropdownMenu
    :items="menuItems"
    :model-value="modelValue"
    trigger-label="更多页签"
    @select="emit('select', $event)"
  >
    <template #item="{ item }">
      <UiTabStatusDot
        v-if="itemByValue.get(item.value)?.status"
        :status="itemByValue.get(item.value)!.status!"
        :title="itemByValue.get(item.value)!.statusTitle"
      />
      <span class="min-w-0 flex-1 truncate">{{ item.label }}</span>
      <span
        v-if="item.value === modelValue"
        class="ml-auto h-[6px] w-[6px] shrink-0 rounded-full bg-accent"
      />
      <button
        v-if="itemByValue.get(item.value)?.closable !== false"
        type="button"
        class="grid h-[16px] w-[16px] shrink-0 place-items-center rounded-[3px] text-text-muted opacity-0 transition-opacity hover:bg-border hover:text-tertiary-strong focus-visible:opacity-100 group-hover:opacity-100 dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-tertiary-dark"
        :aria-label="`关闭 ${item.label}`"
        @click.stop="emit('close', item.value)"
      >
        <UiIcon name="x" :size="10" />
      </button>
    </template>
  </UiDropdownMenu>
</template>
