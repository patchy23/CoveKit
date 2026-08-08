<script setup lang="ts">
/**
 * 时间戳转换 · 秒/毫秒自动识别，时间戳 ⇄ 日期双向互转
 */
import { ref } from "vue";
import { dateToTimestamp, nowSeconds, timestampToResult } from "./useConverter";
import { useCopy } from "@/core/ui/useClipboard";

const { copyText } = useCopy();

const tsInput = ref(String(nowSeconds()));
const tsResult = ref<ReturnType<typeof timestampToResult>>(null);
const tsError = ref("");

const dateInput = ref("");
const dateResult = ref<number | null>(null);
const dateError = ref("");

function convertTs() {
  const r = timestampToResult(tsInput.value);
  tsError.value = r ? "" : "请输入有效的时间戳数字";
  tsResult.value = r;
}

function convertDate() {
  const ms = dateToTimestamp(dateInput.value);
  dateError.value = ms === null ? "无法解析该日期（示例：2023-11-14 22:13:20）" : "";
  dateResult.value = ms;
}

function useNow() {
  tsInput.value = String(nowSeconds());
  convertTs();
}
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <div>
      <div class="mb-[6px] flex items-center justify-between">
        <label class="text-body font-medium text-secondary">时间戳（秒 / 毫秒自动识别）</label>
        <button
          class="rounded-md px-[10px] py-[4px] text-body-sm font-medium text-tertiary-strong transition-colors hover:bg-tertiary-soft dark:text-tertiary-dark dark:hover:bg-tertiary-soft-dark"
          @click="useNow"
        >
          填入当前时间
        </button>
      </div>
      <input
        v-model="tsInput"
        type="text"
        spellcheck="false"
        class="field-input font-mono"
        placeholder="1700000000 或 1700000000000"
      />
    </div>
    <div class="flex items-center gap-[8px]">
      <button class="btn-primary" @click="convertTs">转换为日期</button>
    </div>
    <p
      v-if="tsError"
      class="rounded-sm bg-tertiary-soft px-[12px] py-[9px] text-body text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
    >
      {{ tsError }}
    </p>
    <div v-if="tsResult" class="grid grid-cols-2 gap-[10px]">
      <div class="rounded-md border border-border p-[12px] dark:border-border-dark">
        <div class="text-caption font-semibold text-text-muted">标准时间</div>
        <div class="mt-[4px] font-mono text-body dark:text-primary-dark">
          {{ tsResult.local }}
        </div>
        <button
          class="mt-[6px] text-body-sm text-secondary hover:text-primary dark:text-secondary-dark dark:hover:text-primary-dark"
          @click="copyText(tsResult!.local, '已复制')"
        >
          复制
        </button>
      </div>
      <div class="rounded-md border border-border p-[12px] dark:border-border-dark">
        <div class="text-caption font-semibold text-text-muted">UTC 时间</div>
        <div class="mt-[4px] font-mono text-body dark:text-primary-dark">{{ tsResult.utc }}</div>
        <button
          class="mt-[6px] text-body-sm text-secondary hover:text-primary dark:text-secondary-dark dark:hover:text-primary-dark"
          @click="copyText(tsResult!.utc, '已复制')"
        >
          复制
        </button>
      </div>
      <div class="rounded-md border border-border p-[12px] dark:border-border-dark">
        <div class="text-caption font-semibold text-text-muted">毫秒</div>
        <div class="mt-[4px] font-mono text-body dark:text-primary-dark">{{ tsResult.ms }}</div>
      </div>
      <div class="rounded-md border border-border p-[12px] dark:border-border-dark">
        <div class="text-caption font-semibold text-text-muted">秒</div>
        <div class="mt-[4px] font-mono text-body dark:text-primary-dark">{{ tsResult.sec }}</div>
      </div>
    </div>

    <div class="my-[4px] border-t border-border dark:border-border-dark" />

    <div>
      <label class="mb-[6px] field-label">日期 → 时间戳（毫秒）</label>
      <input
        v-model="dateInput"
        type="text"
        spellcheck="false"
        class="field-input font-mono"
        placeholder="2023-11-14 22:13:20 或 ISO 字符串"
      />
    </div>
    <div class="flex items-center gap-[8px]">
      <button class="btn-secondary" @click="convertDate">转换为时间戳</button>
    </div>
    <p
      v-if="dateError"
      class="rounded-sm bg-tertiary-soft px-[12px] py-[9px] text-body text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
    >
      {{ dateError }}
    </p>
    <div
      v-if="dateResult !== null"
      class="flex items-center justify-between rounded-md border border-border p-[12px] dark:border-border-dark"
    >
      <span class="font-mono text-body text-tertiary-strong dark:text-tertiary-dark">{{
        dateResult
      }}</span>
      <button
        class="rounded-md px-[10px] py-[4px] text-body-sm font-medium text-secondary hover:text-primary dark:text-secondary-dark dark:hover:text-primary-dark"
        @click="copyText(String(dateResult), '已复制')"
      >
        复制
      </button>
    </div>
  </div>
</template>
