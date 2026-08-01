<script setup lang="ts">
/**
 * Base64 编解码 · UTF-8 安全（完整支持中文）
 */
import { ref } from "vue";
import { decodeBase64, encodeBase64, isValidBase64 } from "./useBase64";
import { useCopy } from "@/tools/shared/useClipboard";

const { copyText } = useCopy();

const input = ref("");
const output = ref("");
const errorMsg = ref("");

function runEncode() {
  if (!input.value) return;
  errorMsg.value = "";
  output.value = encodeBase64(input.value);
}

function runDecode() {
  if (!input.value) return;
  if (!isValidBase64(input.value)) {
    errorMsg.value = "输入不是合法的 Base64 字符串";
    output.value = "";
    return;
  }
  try {
    errorMsg.value = "";
    output.value = decodeBase64(input.value);
  } catch {
    errorMsg.value = "解码失败：内容不是有效的 UTF-8 文本";
    output.value = "";
  }
}
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <div>
      <label class="mb-[6px] field-label">输入文本或 Base64</label>
      <textarea
        v-model="input"
        rows="6"
        spellcheck="false"
        class="field-textarea font-mono"
        placeholder="支持中文（UTF-8）"
      />
    </div>
    <div class="flex items-center gap-[8px]">
      <button class="btn-primary" @click="runEncode">编码</button>
      <button class="btn-secondary" @click="runDecode">解码</button>
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
        <button v-if="output" class="btn-ghost" @click="copyText(output, '已复制')">复制</button>
      </div>
      <textarea
        :value="output"
        rows="6"
        readonly
        spellcheck="false"
        class="field-textarea font-mono"
        placeholder="结果将显示在这里"
      />
    </div>
  </div>
</template>
