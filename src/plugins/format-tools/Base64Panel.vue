<script setup lang="ts">
/**
 * Base64 面板 · 文本 ⇄ Base64 双向编解码（格式转换工具子页签；UTF-8 安全）
 * 布局与 JSON/XML 面板一致：输入/输出左右分栏。
 */
import { ref } from 'vue'
import { decodeBase64, encodeBase64 } from './useBase64'
import { useCopy } from '@/core/ui/useClipboard'
import { UiAlert, UiButton, UiCodeEditor, UiToolbar } from '@/core/ui'

const { copyText } = useCopy()

const input = ref('')
const output = ref('')
const error = ref('')

function encode() {
  const r = encodeBase64(input.value)
  error.value = r.ok ? '' : (r.error ?? '编码失败')
  output.value = r.output
}

function decode() {
  const r = decodeBase64(input.value)
  error.value = r.ok ? '' : (r.error ?? '解码失败')
  output.value = r.output
}

function clearAll() {
  input.value = ''
  output.value = ''
  error.value = ''
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[10px]">
    <UiToolbar class="shrink-0">
      <UiButton variant="primary" @click="encode">编码</UiButton>
      <UiButton @click="decode">解码</UiButton>
      <UiButton class="ml-auto" variant="ghost" @click="clearAll">清空</UiButton>
      <UiButton v-if="output" variant="ghost" @click="copyText(output, '已复制结果')">
        复制结果
      </UiButton>
      <span class="text-body-sm text-text-muted dark:text-text-muted-dark">支持中文（UTF-8）</span>
    </UiToolbar>

    <UiAlert v-if="error" tone="danger" class="shrink-0">{{ error }}</UiAlert>

    <div class="grid min-h-0 flex-1 grid-cols-2 gap-[12px]">
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输入</label>
        <UiCodeEditor
          v-model="input"
          class="min-h-0 flex-1"
          placeholder="输入文本或 Base64"
          mode="minimal"
        />
      </div>
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输出</label>
        <UiCodeEditor :model-value="output" readonly language="plaintext" />
      </div>
    </div>
  </div>
</template>
