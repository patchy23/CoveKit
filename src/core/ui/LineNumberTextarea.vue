<script setup lang="ts">
/**
 * LineNumberTextarea · 带行号的文本域（工具输入/输出统一使用）
 * 行号列与内容区滚动同步；wrap="off" 保证行号精确对齐（长行横向滚动）。
 * 只读模式（readonly）用于输出区。
 */
import { computed, ref } from 'vue'

const props = defineProps<{
  modelValue: string
  readonly?: boolean
  placeholder?: string
  /** 固定高度（px）。注意：调用方若传入 flex-1 类，则由 flex 布局分配高度、本值仅作最小保底 */
  minHeight?: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const ta = ref<HTMLTextAreaElement | null>(null)
const gutter = ref<HTMLDivElement | null>(null)

const lineNumbers = computed(() => {
  const n = props.modelValue.split('\n').length
  return Array.from({ length: n }, (_, i) => i + 1)
})

function syncScroll() {
  if (gutter.value && ta.value) gutter.value.scrollTop = ta.value.scrollTop
}

function onInput(e: Event) {
  emit('update:modelValue', (e.target as HTMLTextAreaElement).value)
}
</script>

<template>
  <div
    class="flex w-full overflow-hidden rounded-md border border-border-strong bg-surface-muted transition-colors focus-within:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark"
    :style="minHeight ? { height: minHeight } : {}"
  >
    <!-- 行号列：高度随容器（stretch），内容超高裁剪并由 scrollTop 同步；永不撑高容器 -->
    <div
      ref="gutter"
      class="w-[44px] shrink-0 select-none self-stretch overflow-hidden bg-transparent py-[11px] pr-[10px] text-right font-mono text-body leading-relaxed text-text-muted/50 dark:text-text-muted-dark/50"
      aria-hidden="true"
    >
      <div v-for="n in lineNumbers" :key="n">{{ n }}</div>
    </div>
    <!-- 内容区：wrap off 保证行号对齐；滚动与行号列同步；高度锁定（内容在内部滚动，不拉长页面） -->
    <textarea
      ref="ta"
      class="min-h-[0] flex-1 resize-none border-0 bg-transparent p-[11px] pl-0 font-mono text-body leading-relaxed text-primary outline-none placeholder:text-text-muted dark:text-primary-dark dark:placeholder:text-text-muted-dark"
      :value="modelValue"
      :readonly="readonly"
      :placeholder="placeholder"
      wrap="off"
      spellcheck="false"
      @input="onInput"
      @scroll="syncScroll"
    />
  </div>
</template>
