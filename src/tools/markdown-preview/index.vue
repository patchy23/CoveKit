<script setup lang="ts">
/**
 * Markdown 预览 · 左编辑右预览（GFM，实时渲染）
 */
import { computed, ref } from "vue";
import { renderMarkdown } from "./useMarkdown";

const input = ref(
  "# 欢迎使用 patchyBox\n\n- [x] 实时预览\n- [ ] 支持 GFM\n\n```ts\nconst hello = '世界';\n```\n\n**加粗** 与 `行内代码`"
);

const previewHtml = computed(() => renderMarkdown(input.value));
</script>

<template>
  <div class="grid grid-cols-2 gap-[12px]">
    <div>
      <label class="mb-[6px] field-label">Markdown</label>
      <textarea v-model="input" rows="14" spellcheck="false" class="field-textarea font-mono" />
    </div>
    <div>
      <label class="mb-[6px] field-label">预览</label>
      <div
        class="markdown-body h-full min-h-[320px] resize-y overflow-auto rounded-md border border-border bg-surface-muted p-[14px] leading-relaxed text-primary dark:border-border-dark dark:bg-surface-muted-dark dark:text-primary-dark"
        style="font-size: var(--text-body)"
        v-html="previewHtml"
      />
    </div>
  </div>
</template>

<style>
/* Markdown 预览排版（作用于 v-html 渲染内容） */
.markdown-body h1,
.markdown-body h2,
.markdown-body h3 {
  font-weight: 700;
  margin: 0.8em 0 0.4em;
  line-height: 1.3;
}
.markdown-body h1 {
  font-size: 1.4em;
}
.markdown-body h2 {
  font-size: 1.2em;
}
.markdown-body h3 {
  font-size: 1.05em;
}
.markdown-body p {
  margin: 0.5em 0;
}
.markdown-body ul,
.markdown-body ol {
  padding-left: 1.4em;
  margin: 0.5em 0;
}
.markdown-body ul {
  list-style: disc;
}
.markdown-body ol {
  list-style: decimal;
}
.markdown-body code {
  font-family: var(--font-mono);
  font-size: 0.9em;
  background: var(--color-border);
  border-radius: 4px;
  padding: 1px 5px;
}
.markdown-body pre {
  background: var(--color-neutral-dark);
  border-radius: 8px;
  padding: 12px;
  overflow-x: auto;
  margin: 0.6em 0;
}
.markdown-body pre code {
  background: none;
  padding: 0;
  color: var(--color-primary-dark);
}
.markdown-body blockquote {
  border-left: 3px solid var(--color-tertiary);
  padding-left: 12px;
  color: var(--color-secondary);
  margin: 0.6em 0;
}
.markdown-body table {
  border-collapse: collapse;
  margin: 0.6em 0;
  width: 100%;
}
.markdown-body th,
.markdown-body td {
  border: 1px solid var(--color-border);
  padding: 6px 10px;
  text-align: left;
}
.markdown-body a {
  color: var(--color-tertiary-strong);
  text-decoration: underline;
}
.markdown-body img {
  max-width: 100%;
  border-radius: 8px;
}
</style>
