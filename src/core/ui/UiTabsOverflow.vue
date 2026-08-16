<script setup lang="ts">
/**
 * UiTabsOverflow · 页签溢出收纳（「···」触发器 + 下拉面板，公共组件）
 * 用法：父组件按宽度把放不下的页签传给 items；选中 emit('select')，逐项可关闭 emit('close')。
 * 点击面板外自动收起。样式与 UiSelect 下拉一致（rounded-lg + 边框 + 阴影）。
 */
import { onMounted, onUnmounted, ref } from 'vue'
import UiIcon from './UiIcon.vue'
import type { UiTabItem } from './UiTabs.vue'

withDefaults(
  defineProps<{
    /** 收纳进下拉的页签（顺序即展示顺序） */
    items: UiTabItem[]
    /** 当前激活页签 value（用于高亮） */
    modelValue?: string
    /** 触发器提示文案 */
    title?: string
  }>(),
  { modelValue: '', title: '更多页签' }
)

const emit = defineEmits<{
  (event: 'select', value: string): void
  (event: 'close', value: string): void
}>()

/** 下拉面板开关 */
const open = ref(false)

function select(value: string) {
  emit('select', value)
  open.value = false
}

/** 面板外点击收起（面板自身 mousedown 阻止冒泡） */
function onDocMouseDown() {
  open.value = false
}

onMounted(() => document.addEventListener('mousedown', onDocMouseDown))
onUnmounted(() => document.removeEventListener('mousedown', onDocMouseDown))
</script>

<template>
  <div class="relative shrink-0">
    <button
      type="button"
      class="flex h-[28px] items-center gap-[4px] rounded-md px-[8px] text-body-sm font-medium text-secondary transition-colors hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
      :class="{ 'bg-border text-primary dark:bg-border-dark dark:text-primary-dark': open }"
      :title="`${title}（${items.length}）`"
      @mousedown.stop
      @click.stop="open = !open"
    >
      <UiIcon name="dots" :size="14" />
    </button>

    <div
      v-if="open"
      class="absolute right-0 top-full z-[220] mt-[4px] max-h-[320px] w-[220px] overflow-y-auto rounded-lg border border-border bg-surface py-[4px] shadow-[0_16px_40px_rgba(16,24,40,0.18)] dark:border-border-dark dark:bg-surface-dark"
      @mousedown.stop
    >
      <div
        v-for="item in items"
        :key="item.value"
        class="group flex cursor-pointer items-center gap-[8px] px-[10px] py-[7px] text-body-sm transition-colors"
        :class="
          item.value === modelValue
            ? 'bg-tertiary-soft font-medium text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
            : 'text-secondary hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark'
        "
        @click="select(item.value)"
      >
        <span
          v-if="item.status"
          class="h-[7px] w-[7px] shrink-0 rounded-full"
          :class="{
            'bg-success-strong dark:bg-success-dark': item.status === 'success',
            'bg-danger-strong dark:bg-danger-dark': item.status === 'danger',
            'bg-text-muted dark:bg-text-muted-dark': item.status === 'neutral',
          }"
          :title="item.statusTitle"
        />
        <span class="min-w-0 flex-1 truncate">{{ item.label }}</span>
        <span
          v-if="item.badge !== undefined"
          class="rounded-full bg-border px-[6px] text-caption dark:bg-border-dark"
          >{{ item.badge }}</span
        >
        <button
          v-if="item.closable"
          type="button"
          class="grid h-[16px] w-[16px] shrink-0 place-items-center rounded-[3px] text-caption text-text-muted opacity-0 transition-opacity hover:bg-border hover:text-tertiary-strong group-hover:opacity-100 dark:text-text-muted-dark dark:hover:bg-border-dark dark:hover:text-tertiary-dark"
          :title="`关闭${item.label}`"
          @click.stop="emit('close', item.value)"
        >
          ×
        </button>
      </div>
    </div>
  </div>
</template>
