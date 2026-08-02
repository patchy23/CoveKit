<script setup lang="ts">
/**
 * HighlightedOutput · 语法高亮 + 行号的只读输出区（JSON/XML 格式化输出用）
 * 行号按原文行数渲染，与高亮内容区滚动同步；高度由调用方 flex 决定。
 */
import { computed, ref } from "vue";

const props = defineProps<{
  /** 原文（用于计算行号与空态占位） */
  text: string;
  /** 语法高亮后的 HTML（v-html 渲染）；为空时显示原文纯文本 */
  html: string;
}>();

const pre = ref<HTMLElement | null>(null);
const gutter = ref<HTMLDivElement | null>(null);

const lineNumbers = computed(() => {
  const n = props.text.split("\n").length;
  return Array.from({ length: n }, (_, i) => i + 1);
});

function syncScroll() {
  if (gutter.value && pre.value) gutter.value.scrollTop = pre.value.scrollTop;
}

/** 点击输出区：选中全部内容（便于 Ctrl+C 复制） */
function selectAll(e: MouseEvent) {
  const el = e.currentTarget as HTMLElement;
  const range = document.createRange();
  range.selectNodeContents(el);
  const sel = window.getSelection();
  sel?.removeAllRanges();
  sel?.addRange(range);
}
</script>

<template>
  <div
    class="flex min-h-0 w-full overflow-hidden rounded-md border border-border bg-surface-muted dark:border-border-dark dark:bg-surface-muted-dark"
  >
    <!-- 行号列（与内容区同行高；超高裁剪 + scrollTop 同步，不撑高容器） -->
    <div
      ref="gutter"
      class="w-[44px] shrink-0 select-none self-stretch overflow-hidden border-r border-border/60 bg-transparent py-[13px] pr-[10px] text-right font-mono text-body leading-relaxed text-text-muted/50 dark:border-border-dark/60 dark:text-text-muted-dark/50"
      aria-hidden="true"
    >
      <div v-for="n in lineNumbers" :key="n">{{ n }}</div>
    </div>
    <!-- 高亮内容区（内部滚动；点击全选便于复制） -->
    <pre
      ref="pre"
      class="min-h-0 flex-1 cursor-text overflow-auto p-[13px] pl-[13px] font-mono text-body leading-relaxed"
      title="点击全选内容，Ctrl+C 复制"
      @scroll="syncScroll"
      @click="selectAll"
    ><code v-if="html" class="hljs" v-html="html" /><span v-else class="text-text-muted dark:text-text-muted-dark">{{ text || "格式化结果将显示在这里" }}</span></pre>
  </div>
</template>
