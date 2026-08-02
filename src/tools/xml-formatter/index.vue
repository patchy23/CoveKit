<script setup lang="ts">
/**
 * XML 格式化 · 格式化/压缩/校验（输入输出均锁定高度，内置滚动）
 */
import { computed, ref } from "vue";
import hljs from "highlight.js";
import { formatXml, minifyXml, isValidXml } from "./useXml";
import LineNumberTextarea from "@/tools/shared/LineNumberTextarea.vue";
import { useCopy } from "@/tools/shared/useClipboard";

const { copyText } = useCopy();

const input = ref(
  '<?xml version="1.0" encoding="UTF-8"?>\n<config>\n  <app name="patchyBox">\n    <version>0.1.0</version>\n  </app>\n</config>'
);
const output = ref("");
const error = ref("");

const highlighted = computed(() => {
  if (!output.value) return "";
  try {
    return hljs.highlight(output.value, { language: "xml", ignoreIllegals: true }).value;
  } catch {
    return "";
  }
});

function format() {
  error.value = "";
  const r = formatXml(input.value);
  if (r.ok) {
    output.value = r.output ?? "";
  } else {
    output.value = "";
    error.value = r.error ?? "格式化失败";
  }
}

function minify() {
  error.value = "";
  const t = input.value.trim();
  if (!t) return;
  if (!isValidXml(t)) {
    error.value = "XML 解析失败";
    return;
  }
  output.value = minifyXml(t);
}
</script>

<template>
  <div class="flex max-w-[1000px] flex-col gap-[12px]">
    <div class="flex items-center gap-[10px]">
      <button class="btn-primary" @click="format">格式化</button>
      <button class="btn-secondary" @click="minify">压缩</button>
      <button v-if="output" class="btn-ghost" @click="copyText(output, '已复制格式化结果')">
        复制结果
      </button>
      <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
        支持 XML 声明 / 注释 / 自闭合标签
      </span>
    </div>

    <p v-if="error" class="text-body-sm text-tertiary-strong dark:text-tertiary-dark">
      {{ error }}
    </p>

    <div class="grid grid-cols-2 gap-[12px]">
      <div class="flex flex-col">
        <label class="mb-[6px] field-label">输入 XML</label>
        <LineNumberTextarea v-model="input" min-height="360px" />
      </div>
      <div class="flex flex-col">
        <label class="mb-[6px] field-label">输出</label>
        <div
          class="h-[360px] overflow-auto rounded-md border border-border bg-surface-muted p-[11px] font-mono text-body leading-relaxed dark:border-border-dark dark:bg-surface-muted-dark"
        >
          <code v-if="highlighted" class="hljs" v-html="highlighted" />
          <span v-else class="text-text-muted dark:text-text-muted-dark">
            {{ output || "格式化结果将显示在这里" }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>
