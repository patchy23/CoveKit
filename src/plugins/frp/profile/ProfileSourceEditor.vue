<script setup lang="ts">
/**
 * ProfileSourceEditor · 源码模式（TOML 原文编辑）
 * 直接编辑用户文件原文；`frpc verify` 的错误行在编辑器下方列出（带行号，点击可定位无法实现，
 * 因编辑器未暴露跳转 API，故按行号排序展示供人工对照）。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiCodeEditor } from '@/core/ui'
import type { FrpVerifyError } from '../contracts'

const props = defineProps<{
  /** TOML 原文 */
  modelValue: string
  /** 文件名（语言识别与状态栏展示） */
  fileName: string
  /** 最近一次校验错误（空数组 = 未校验或通过） */
  errors: FrpVerifyError[]
  /** 是否已校验（用于区分「未校验」与「校验通过」） */
  verified: boolean
}>()
const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const { t } = useI18n()

/** 是否含注释（表单保存会丢失，这里提示用户源码模式更安全） */
const hasComments = computed(() =>
  props.modelValue.split('\n').some((line) => line.trimStart().startsWith('#'))
)

/** 错误行号文本（无行号时只显示消息） */
function errorLabel(item: FrpVerifyError): string {
  if (item.line === undefined) return item.message
  return item.column === undefined
    ? t('frp.verifyErrorLine', { line: item.line, message: item.message })
    : t('frp.verifyErrorLineCol', { line: item.line, column: item.column, message: item.message })
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div class="min-h-0 flex-1">
      <UiCodeEditor
        :model-value="props.modelValue"
        language="toml"
        :filename="props.fileName"
        height="100%"
        :lint="false"
        status-bar
        @update:model-value="emit('update:modelValue', $event)"
      />
    </div>

    <!-- 校验结果（错误逐条列出；通过时给一行确认） -->
    <div
      v-if="props.errors.length > 0"
      class="max-h-[132px] shrink-0 overflow-y-auto border-t border-danger-soft bg-danger-soft px-[10px] py-[6px] dark:border-danger-dark dark:bg-danger-soft-dark"
    >
      <p class="text-caption font-medium text-danger-strong dark:text-danger-dark">
        {{ t('frp.verifyFailedCount', { count: props.errors.length }) }}
      </p>
      <p
        v-for="(item, index) in props.errors"
        :key="index"
        class="mt-[2px] font-mono text-caption text-danger-strong dark:text-danger-dark"
      >
        {{ errorLabel(item) }}
      </p>
    </div>
    <p
      v-else-if="props.verified"
      class="shrink-0 border-t border-border px-[10px] py-[5px] text-caption text-success-strong dark:border-border-dark dark:text-success-dark"
    >
      {{ t('frp.verifyPassed') }}
    </p>

    <p
      v-if="hasComments"
      class="shrink-0 border-t border-border px-[10px] py-[5px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      {{ t('frp.sourceCommentHint') }}
    </p>
  </div>
</template>
