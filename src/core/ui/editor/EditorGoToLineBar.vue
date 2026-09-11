<script setup lang="ts">
/**
 * 跳转行浮层（Ctrl+G）
 *
 * 只收集目标行号并发出意图；越界收敛交给编辑器侧（`goToLine` 内部按总行数收敛）。
 */
import { computed, ref } from 'vue'
import { UiButton, UiInput } from '@/core/ui'

const props = defineProps<{
  /** 当前行号（打开时预填） */
  currentLine: number
  /** 文档总行数（提示上限并约束输入） */
  totalLines: number
}>()

const emit = defineEmits<{
  (event: 'confirm', line: number): void
  (event: 'close'): void
}>()

const value = ref(String(props.currentLine))
const input = ref<{ focus?: () => void; select?: () => void } | null>(null)

/** 输入解析出的行号（非法或非正数时为 null，按钮随之禁用） */
const parsedLine = computed(() => {
  const parsed = Number.parseInt(value.value, 10)
  return Number.isFinite(parsed) && parsed > 0 ? Math.min(parsed, props.totalLines) : null
})

function confirm(): void {
  if (parsedLine.value === null) return
  emit('confirm', parsedLine.value)
}

/** 聚焦并全选，便于直接覆盖输入 */
function focus(): void {
  input.value?.focus?.()
  input.value?.select?.()
}

defineExpose({ focus })
</script>

<template>
  <div
    class="flex w-[248px] items-center gap-2 rounded-md border border-border bg-surface p-2 shadow-lg dark:border-border-dark dark:bg-surface-dark"
  >
    <div class="w-[96px]">
      <UiInput
        ref="input"
        v-model="value"
        size="sm"
        placeholder="行号"
        @keydown.enter.prevent="confirm"
        @keydown.esc.stop="emit('close')"
      />
    </div>
    <span class="flex-1 text-caption whitespace-nowrap text-text-muted dark:text-text-muted-dark">
      共 {{ totalLines }} 行
    </span>
    <UiButton size="sm" variant="primary" :disabled="parsedLine === null" @click="confirm">
      跳转
    </UiButton>
  </div>
</template>
