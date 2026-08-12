<script setup lang="ts">
/**
 * XML 格式化 · 格式化/压缩/校验（与 JSON 格式化同款布局：占满工作区 + 内部滚动）
 */
import { ref } from 'vue'
import { formatXml, minifyXml } from './useXml'
import LineNumberTextarea from '@/core/ui/LineNumberTextarea.vue'
import CodeViewer from '@/core/ui/CodeViewer.vue'
import { useCopy } from '@/core/ui/useClipboard'
import { UiAlert, UiButton, UiToolbar } from '@/core/ui'

const { copyText } = useCopy()

const input = ref(
  '<?xml version="1.0" encoding="UTF-8"?>\n<config>\n  <app name="patchyBox">\n    <version>0.1.0</version>\n  </app>\n</config>'
)
const output = ref('')
const error = ref('')

function format() {
  error.value = ''
  const r = formatXml(input.value)
  if (r.ok) {
    output.value = r.output ?? ''
    if (r.loose) {
      error.value = '宽松模式：命名空间或结构未通过严格校验，已按标签缩进格式化'
    }
  } else {
    output.value = ''
    error.value = r.error ?? '格式化失败'
  }
}

function minify() {
  error.value = ''
  const t = input.value.trim()
  if (!t) return
  output.value = minifyXml(t)
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[10px]">
    <UiToolbar class="shrink-0">
      <UiButton variant="primary" @click="format">格式化</UiButton>
      <UiButton @click="minify">压缩</UiButton>
      <UiButton
        class="ml-auto"
        variant="ghost"
        :disabled="!output"
        @click="copyText(output, '已复制格式化结果')"
      >
        复制结果
      </UiButton>
      <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
        支持声明 / 注释 / 自闭合标签
      </span>
    </UiToolbar>

    <UiAlert v-if="error" tone="warning" class="shrink-0">{{ error }}</UiAlert>

    <div class="grid min-h-0 flex-1 grid-cols-2 gap-[12px]">
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输入 XML</label>
        <LineNumberTextarea v-model="input" class="min-h-0 flex-1" />
      </div>
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输出</label>
        <CodeViewer :doc="output" lang="xml" />
      </div>
    </div>
  </div>
</template>
