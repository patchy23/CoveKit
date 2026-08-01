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
      <label class="mb-[6px] field-label">输入 JSON</label>
      <textarea
        v-model="input"
        rows="8"
        spellcheck="false"
        class="field-textarea font-mono"
        placeholder='输入 JSON，如 {"a": 1}'
      />
    </div>
    <div class="flex items-center gap-[8px]">
      <button class="btn-primary" @click="runFormat">格式化</button>
      <button class="btn-secondary" @click="runMinify">压缩</button>
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
        <label class="text-body font-medium text-secondary">输出</label>
        <button v-if="output" class="btn-ghost" @click="copyText(output, 'JSON 已复制')">
          复制
        </button>
      </div>
      <textarea
        :value="output"
        rows="8"
        readonly
        spellcheck="false"
        class="field-textarea font-mono"
        placeholder="格式化结果将显示在这里"
      />
    </div>
  </div>
</template>
