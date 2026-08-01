<script setup lang="ts">
/**
 * JSON 格式化 · 格式化/压缩/校验 + 错误行号定位
 * 输入带行号；输出语法高亮（highlight.js）。
 */
import { computed, ref } from "vue";
import hljs from "highlight.js";
import { formatJson, minifyJson } from "./useFormat";
import { useCopy } from "@/tools/shared/useClipboard";
import LineNumberTextarea from "@/tools/shared/LineNumberTextarea.vue";
import { useSettingsStore } from "@/stores/settings";

const settings = useSettingsStore();
const { copyText } = useCopy();

const input = ref('{\n  "name": "patchyBox",\n  "tools": 8\n}');
const output = ref("");
const errorMsg = ref("");

function runFormat() {
  const r = formatJson(input.value, settings.getToolSetting("json-formatter", "indent", 2));
  errorMsg.value = r.error
    ? `${r.error.message}（第 ${r.error.line} 行，第 ${r.error.col} 列）`
    : "";
  output.value = r.output;
}

function runMinify() {
  const r = minifyJson(input.value);
  errorMsg.value = r.error
    ? `${r.error.message}（第 ${r.error.line} 行，第 ${r.error.col} 列）`
    : "";
  output.value = r.output;
}

function clearAll() {
  input.value = "";
  output.value = "";
  errorMsg.value = "";
}

/** 输出语法高亮（解析失败时回退为 HTML 转义纯文本） */
const highlighted = computed(() => {
  if (!output.value) return "";
  try {
    return hljs.highlight(output.value, { language: "json", ignoreIllegals: true }).value;
  } catch {
    return output.value.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }
});
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <div>
      <label class="mb-[6px] field-label">输入 JSON</label>
      <LineNumberTextarea v-model="input" min-height="180px" placeholder='输入 JSON，如 {"a": 1}' />
    </div>
    <div class="flex items-center gap-[8px]">
      <button class="btn-primary" @click="runFormat">格式化</button>
      <button class="btn-secondary" @click="runMinify">压缩</button>
      <button class="ml-auto btn-ghost" @click="clearAll">清空</button>
    </div>
    <p
      v-if="errorMsg"
      class="rounded-sm bg-tertiary-soft px-[12px] py-[9px] text-body text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
    >
      {{ errorMsg }}
    </p>
    <div>
      <div class="mb-[6px] flex items-center justify-between">
        <label class="field-label">输出</label>
        <button v-if="output" class="btn-ghost" @click="copyText(output, 'JSON 已复制')">
          复制
        </button>
      </div>
      <!-- pre 内不可换行缩进（pre 保留空白，会导致输出前出现空格） -->
      <pre
        class="min-h-[180px] overflow-auto rounded-md border border-border bg-surface-muted p-[13px] font-mono text-body leading-relaxed dark:border-border-dark dark:bg-surface-muted-dark"
      ><code v-if="output" class="hljs" v-html="highlighted" /><span v-else class="text-text-muted dark:text-text-muted-dark">格式化结果将显示在这里</span></pre>
    </div>
  </div>
</template>
