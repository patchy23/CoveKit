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
      <label class="mb-[6px] block text-body font-medium text-secondary">输入文本或 Base64</label>
      <textarea
        v-model="input"
        rows="6"
        spellcheck="false"
        class="w-full resize-y rounded-md border border-border-strong bg-surface-muted p-[11px] font-mono text-body leading-relaxed text-primary outline-none transition-colors focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
        placeholder="支持中文（UTF-8）"
      />
    </div>
    <div class="flex items-center gap-[8px]">
      <button
        class="h-[38px] rounded-md bg-tertiary-strong px-[18px] text-body font-medium text-on-tertiary transition-[filter] hover:brightness-110 dark:bg-tertiary-dark dark:text-on-tertiary-dark"
        @click="runEncode"
      >
        编码
      </button>
      <button
        class="h-[38px] rounded-md border border-border-strong bg-surface px-[18px] text-body font-medium text-primary transition-colors hover:bg-surface-muted dark:border-border-strong-dark dark:bg-surface-dark dark:text-primary-dark dark:hover:bg-surface-muted-dark"
        @click="runDecode"
      >
        解码
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
        <label class="text-body font-medium text-secondary">输出</label>
        <button
          v-if="output"
          class="rounded-md px-[10px] py-[4px] text-body-sm font-medium text-secondary transition-colors hover:bg-border hover:text-primary dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          @click="copyText(output, '已复制')"
        >
          复制
        </button>
      </div>
      <textarea
        :value="output"
        rows="6"
        readonly
        spellcheck="false"
        class="w-full resize-y rounded-md border border-border bg-surface-muted p-[11px] font-mono text-body leading-relaxed text-primary outline-none dark:border-border-dark dark:bg-surface-muted-dark dark:text-primary-dark"
        placeholder="结果将显示在这里"
      />
    </div>
  </div>
</template>
