<script setup lang="ts">
/**
 * JSON 格式化 · 格式化/压缩/校验 + 错误行号定位
 */
import { ref } from "vue";
import { formatJson, minifyJson } from "./useFormat";
import { useCopy } from "@/tools/shared/useClipboard";
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
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <div>
      <label class="mb-[6px] block text-body-sm font-semibold text-secondary">输入 JSON</label>
      <textarea
        v-model="input"
        rows="8"
        spellcheck="false"
        class="w-full resize-y rounded-md border border-border-strong bg-surface-muted p-[11px] font-mono text-body leading-relaxed text-primary outline-none transition-colors focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
        placeholder='输入 JSON，如 {"a": 1}'
      />
    </div>
    <div class="flex items-center gap-[8px]">
      <button
        class="h-[38px] rounded-md bg-tertiary-strong px-[18px] text-body font-semibold text-on-tertiary transition-[filter] hover:brightness-110 dark:bg-tertiary-dark dark:text-on-tertiary-dark"
        @click="runFormat"
      >
        格式化
      </button>
      <button
        class="h-[38px] rounded-md border border-border-strong bg-surface px-[18px] text-body font-semibold text-primary transition-colors hover:bg-surface-muted dark:border-border-strong-dark dark:bg-surface-dark dark:text-primary-dark dark:hover:bg-surface-muted-dark"
        @click="runMinify"
      >
        压缩
      </button>
      <button
        class="ml-auto rounded-md px-[12px] py-[9px] text-body font-medium text-secondary transition-colors hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        @click="
          input = '';
          output = '';
          errorMsg = '';
        "
      >
        清空
      </button>
    </div>
    <p
      v-if="errorMsg"
      class="rounded-sm bg-tertiary-soft px-[12px] py-[9px] text-body text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
    >
      {{ errorMsg }}
    </p>
    <div>
      <div class="mb-[6px] flex items-center justify-between">
        <label class="text-body-sm font-semibold text-secondary">输出</label>
        <button
          v-if="output"
          class="rounded-md px-[10px] py-[4px] text-body-sm font-medium text-secondary transition-colors hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          @click="copyText(output, 'JSON 已复制')"
        >
          复制
        </button>
      </div>
      <textarea
        :value="output"
        rows="8"
        readonly
        spellcheck="false"
        class="w-full resize-y rounded-md border border-border bg-surface-muted p-[11px] font-mono text-body leading-relaxed text-primary outline-none dark:border-border-dark dark:bg-surface-muted-dark dark:text-primary-dark"
        placeholder="格式化结果将显示在这里"
      />
    </div>
  </div>
</template>
