<script setup lang="ts">
/**
 * 时间戳面板 · 秒/毫秒自动识别，时间戳 ⇄ 日期双向互转（格式转换工具子页签）
 * 免按钮交互（2026-09-18）：输入停顿 300ms 自动转换；「填入当前时间」立即重算。
 */
import { ref } from 'vue'
import { dateToTimestamp, nowSeconds, timestampToResult } from './useConverter'
import { useAutoConvert } from './useAutoConvert'
import { useCopy } from '@/core/feedback/useCopy'
import { UiAlert, UiButton, UiField, UiInput, UiPanel } from '@/core/ui'

const { copyText } = useCopy()

const tsInput = ref(String(nowSeconds()))
const tsResult = ref<ReturnType<typeof timestampToResult>>(null)
const tsError = ref('')

const dateInput = ref('')
const dateResult = ref<number | null>(null)
const dateError = ref('')

/** 时间戳 → 日期；空输入清空结果，不报错 */
function convertTs() {
  const text = tsInput.value.trim()
  if (!text) {
    tsResult.value = null
    tsError.value = ''
    return
  }
  const r = timestampToResult(text)
  tsError.value = r ? '' : '请输入有效的时间戳数字'
  tsResult.value = r
}

/** 日期 → 时间戳（毫秒）；空输入清空结果，不报错 */
function convertDate() {
  const text = dateInput.value.trim()
  if (!text) {
    dateResult.value = null
    dateError.value = ''
    return
  }
  const ms = dateToTimestamp(text)
  dateError.value = ms === null ? '无法解析该日期（示例：2023-11-14 22:13:20）' : ''
  dateResult.value = ms
}

// 两个输入框各自独立的防抖调度
const tsAuto = useAutoConvert(convertTs)
tsAuto.watchInput(tsInput)
const dateAuto = useAutoConvert(convertDate)
dateAuto.watchInput(dateInput)
// 挂载默认填入当前时间戳，立即出结果
tsAuto.runNow()

function useNow() {
  tsInput.value = String(nowSeconds())
  // 按钮点击属明确动作，立即重算不等防抖
  tsAuto.runNow()
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
    <UiAlert v-if="tsError" tone="danger">{{ tsError }}</UiAlert>
    <div v-if="tsResult" class="grid grid-cols-2 gap-[10px]">
      <UiPanel padding="sm">
        <div class="text-caption font-semibold text-text-muted">标准时间</div>
        <div class="mt-[4px] select-text font-mono text-body dark:text-primary-dark">
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
        <div class="mt-[4px] select-text font-mono text-body dark:text-primary-dark">
          {{ tsResult.utc }}
        </div>
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
        <div class="mt-[4px] select-text font-mono text-body dark:text-primary-dark">
          {{ tsResult.ms }}
        </div>
      </UiPanel>
      <UiPanel padding="sm">
        <div class="text-caption font-semibold text-text-muted">秒</div>
        <div class="mt-[4px] select-text font-mono text-body dark:text-primary-dark">
          {{ tsResult.sec }}
        </div>
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
    <UiAlert v-if="dateError" tone="danger">{{ dateError }}</UiAlert>
    <div
      v-if="dateResult !== null"
      class="flex items-center justify-between rounded-md border border-border p-[12px] dark:border-border-dark"
    >
      <span class="select-text font-mono text-body text-tertiary-strong dark:text-tertiary-dark">{{
        dateResult
      }}</span>
      <UiButton variant="ghost" size="sm" @click="copyText(String(dateResult), '已复制')"
        >复制</UiButton
      >
    </div>
  </div>
</template>
