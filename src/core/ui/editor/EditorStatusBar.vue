<script setup lang="ts">
import UiTooltip from '../UiTooltip.vue'
/**
 * 编辑器状态栏（纯展示，无交互）
 *
 * 左侧：行列 / 选中字符数 / 语言 / 缩进 / 规模 / 编码；右侧：大文件降级提示。
 * 文案拼装全部来自 `status.ts` 的纯函数，本组件只负责布局与语义 token 取色。
 */
import { computed } from 'vue'
import type { EditorStatusText } from './status'

const props = defineProps<{
  /** 已拼装好的状态分项 */
  status: EditorStatusText
}>()

/** 左侧可见分项（跳过 null 项） */
const items = computed(() =>
  [
    props.status.position,
    props.status.selection,
    props.status.language,
    props.status.indent,
    props.status.size,
    props.status.encoding,
  ].filter((item): item is string => Boolean(item))
)
</script>

<template>
  <div
    class="flex h-[26px] shrink-0 items-center justify-between gap-3 border-t border-border bg-surface-muted px-3 text-caption text-text-muted select-none dark:border-border-dark dark:bg-surface-muted-dark dark:text-text-muted-dark"
  >
    <div class="flex min-w-0 items-center gap-3 overflow-hidden">
      <span v-for="item in items" :key="item" class="whitespace-nowrap">{{ item }}</span>
    </div>
    <UiTooltip v-if="status.degrade" content="大文件降级提示">
      <span class="whitespace-nowrap text-tertiary-strong dark:text-tertiary-dark">
        {{ status.degrade }}
      </span>
    </UiTooltip>
  </div>
</template>
