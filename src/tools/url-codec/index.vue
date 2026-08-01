<script setup lang="ts">
/**
 * URL 编解码 · encodeURIComponent / decodeURIComponent
 */
import { ref } from "vue";
import { decodeUrl, encodeUrl, hasEncodedFragment } from "./useUrlCodec";
import { useCopy } from "@/tools/shared/useClipboard";

const { copyText } = useCopy();

const input = ref("");
const output = ref("");
const errorMsg = ref("");

function runEncode() {
  if (!input.value) return;
  errorMsg.value = "";
  output.value = encodeUrl(input.value).output;
}

function runDecode() {
  if (!input.value) return;
  const r = decodeUrl(input.value);
  errorMsg.value = r.error ?? "";
  output.value = r.output;
}
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <div>
      <label class="mb-[6px] field-label">输入内容</label>
      <textarea
        v-model="input"
        rows="6"
        spellcheck="false"
        class="field-textarea font-mono"
        placeholder="https://example.com/搜索?q=patchy box"
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
    <p v-if="!errorMsg && input && !hasEncodedFragment(input)" class="text-body-sm text-text-muted">
      提示：当前输入不含 %xx 片段，解码前请确认内容已编码
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
