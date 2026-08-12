<script setup lang="ts">
/**
 * 时间戳转换 · 秒/毫秒自动识别，时间戳 ⇄ 日期双向互转
 */
import { ref } from 'vue'
import { dateToTimestamp, nowSeconds, timestampToResult } from './useConverter'
import { useCopy } from '@/core/ui/useClipboard'
import { UiAlert, UiButton, UiField, UiInput, UiPanel } from '@/core/ui'

const { copyText } = useCopy()

const tsInput = ref(String(nowSeconds()))
const tsResult = ref<ReturnType<typeof timestampToResult>>(null)
const tsError = ref('')

const dateInput = ref('')
const dateResult = ref<number | null>(null)
const dateError = ref('')

function convertTs() {
  const r = timestampToResult(tsInput.value)
  tsError.value = r ? '' : '请输入有效的时间戳数字'
  tsResult.value = r
}

function convertDate() {
  const ms = dateToTimestamp(dateInput.value)
  dateError.value = ms === null ? '无法解析该日期（示例：2023-11-14 22:13:20）' : ''
  dateResult.value = ms
}

function useNow() {
  tsInput.value = String(nowSeconds())
  convertTs()
}
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <UiField label="时间戳（秒 / 毫秒自动识别）">
      <div class="mb-[6px] flex items-center justify-between">
        <span />
        <UiButton variant="ghost" size="sm" @click="useNow">填入当前时间</UiButton>
      </div>
      <UiInput
        v-model="tsInput"
        spellcheck="false"
        class="font-mono"
        placeholder="1700000000 或 1700000000000"
      />
    </UiField>
    <div class="flex items-center gap-[8px]">
      <UiButton variant="primary" @click="convertTs">转换为日期</UiButton>
    </div>
    <UiAlert v-if="tsError" tone="danger">{{ tsError }}</UiAlert>
    <div v-if="tsResult" class="grid grid-cols-2 gap-[10px]">
      <UiPanel padding="sm">
        <div class="text-caption font-semibold text-text-muted">标准时间</div>
        <div class="mt-[4px] font-mono text-body dark:text-primary-dark">
          {{ tsResult.local }}
        </div>
        <UiButton
          class="mt-[6px]"
          variant="ghost"
          size="sm"
          @click="copyText(tsResult!.local, '已复制')"
          >复制</UiButton
        >
      </UiPanel>
      <UiPanel padding="sm">
        <div class="text-caption font-semibold text-text-muted">UTC 时间</div>
        <div class="mt-[4px] font-mono text-body dark:text-primary-dark">{{ tsResult.utc }}</div>
        <UiButton
          class="mt-[6px]"
          variant="ghost"
          size="sm"
          @click="copyText(tsResult!.utc, '已复制')"
          >复制</UiButton
        >
      </UiPanel>
      <UiPanel padding="sm">
        <div class="text-caption font-semibold text-text-muted">毫秒</div>
        <div class="mt-[4px] font-mono text-body dark:text-primary-dark">{{ tsResult.ms }}</div>
      </UiPanel>
      <UiPanel padding="sm">
        <div class="text-caption font-semibold text-text-muted">秒</div>
        <div class="mt-[4px] font-mono text-body dark:text-primary-dark">{{ tsResult.sec }}</div>
      </UiPanel>
    </div>

    <div class="my-[4px] border-t border-border dark:border-border-dark" />

    <UiField label="日期 → 时间戳（毫秒）">
      <UiInput
        v-model="dateInput"
        spellcheck="false"
        class="font-mono"
        placeholder="2023-11-14 22:13:20 或 ISO 字符串"
      />
    </UiField>
    <div class="flex items-center gap-[8px]">
      <UiButton @click="convertDate">转换为时间戳</UiButton>
    </div>
    <UiAlert v-if="dateError" tone="danger">{{ dateError }}</UiAlert>
    <div
      v-if="dateResult !== null"
      class="flex items-center justify-between rounded-md border border-border p-[12px] dark:border-border-dark"
    >
      <span class="font-mono text-body text-tertiary-strong dark:text-tertiary-dark">{{
        dateResult
      }}</span>
      <UiButton variant="ghost" size="sm" @click="copyText(String(dateResult), '已复制')"
        >复制</UiButton
      >
    </div>
  </div>
</template>
