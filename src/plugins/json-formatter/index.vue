<script setup lang="ts">
/**
 * JSON 格式化 · 格式化/压缩/校验 + 错误行号定位
 * 布局：输入/输出左右分栏，占满工作区高度；文本框内部滚动（不拉长页面）。
 */
import { ref } from 'vue'
import { formatJson, minifyJson } from './useFormat'
import { useCopy } from '@/core/ui/useClipboard'
import LineNumberTextarea from '@/core/ui/LineNumberTextarea.vue'
import CodeViewer from '@/core/ui/CodeViewer.vue'
import { useSettingsStore } from '@/stores/settings'
import { UiAlert, UiButton, UiToolbar } from '@/core/ui'

const settings = useSettingsStore()
const { copyText } = useCopy()

const input = ref('{\n  "name": "patchyBox",\n  "tools": 8\n}')
const output = ref('')
const errorMsg = ref('')

function runFormat() {
  const r = formatJson(input.value, settings.getToolSetting('json-formatter', 'indent', 2))
  errorMsg.value = r.error
    ? `${r.error.message}（第 ${r.error.line} 行，第 ${r.error.col} 列）`
    : ''
  output.value = r.output
}

function runMinify() {
  const r = minifyJson(input.value)
  errorMsg.value = r.error
    ? `${r.error.message}（第 ${r.error.line} 行，第 ${r.error.col} 列）`
    : ''
  output.value = r.output
}

function clearAll() {
  input.value = ''
  output.value = ''
  errorMsg.value = ''
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[10px]">
    <UiToolbar class="shrink-0">
      <UiButton variant="primary" @click="runFormat">格式化</UiButton>
      <UiButton @click="runMinify">压缩</UiButton>
      <UiButton class="ml-auto" variant="ghost" @click="clearAll">清空</UiButton>
      <UiButton v-if="output" variant="ghost" @click="copyText(output, 'JSON 已复制')">
        复制结果
      </UiButton>
    </UiToolbar>

    <UiAlert v-if="errorMsg" tone="danger" class="shrink-0">{{ errorMsg }}</UiAlert>

    <div class="grid min-h-0 flex-1 grid-cols-2 gap-[12px]">
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输入 JSON</label>
        <LineNumberTextarea
          v-model="input"
          class="min-h-0 flex-1"
          placeholder='输入 JSON，如 {"a": 1}'
        />
      </div>
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输出</label>
        <CodeViewer :doc="output" lang="json" />
      </div>
    </div>
  </div>
</template>
