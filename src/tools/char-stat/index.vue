<script setup lang="ts">
/**
 * 字符统计 · 实时统计（字符数/去空白/词数/行数/字节数/中日韩）
 */
import { computed, ref } from "vue";
import { statText } from "./useCharStat";

const input = ref("");
const stats = computed(() => statText(input.value));

const items = [
  { key: "chars", label: "字符数" },
  { key: "charsNoSpace", label: "去空白" },
  { key: "words", label: "词数" },
  { key: "lines", label: "行数" },
  { key: "bytes", label: "字节数" },
  { key: "cjk", label: "中日韩" },
] as const;
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <div>
      <label class="mb-[6px] block text-body-sm font-semibold text-secondary">输入文本</label>
      <textarea
        v-model="input"
        rows="10"
        spellcheck="false"
        class="w-full resize-y rounded-md border border-border-strong bg-surface-muted p-[11px] font-mono text-body leading-relaxed text-primary outline-none transition-colors focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
        placeholder="在此输入或粘贴文本，统计实时更新"
      />
    </div>
    <div class="grid grid-cols-3 gap-[10px]">
      <div
        v-for="item in items"
        :key="item.key"
        class="rounded-md border border-border p-[12px] text-center dark:border-border-dark"
      >
        <div
          class="text-display font-bold tabular-nums text-tertiary-strong dark:text-tertiary-dark"
        >
          {{ stats[item.key] }}
        </div>
        <div class="mt-[2px] text-caption font-medium text-text-muted">{{ item.label }}</div>
      </div>
    </div>
  </div>
</template>
