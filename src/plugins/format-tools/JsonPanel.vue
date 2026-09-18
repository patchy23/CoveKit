<script setup lang="ts">
/**
 * JSON 面板 · 格式化/压缩/校验 + 错误行号定位（格式转换工具子页签）
 * 布局：输入/输出左右分栏，占满高度；文本框内部滚动。缩进固定 2 空格（2026-09-14 起不再做成设置项）。
 * 免按钮交互（2026-09-18）：输入停顿 300ms 自动转换，模式切换立即重算（参考 DevToys / IT Tools）。
 */
import { ref, watch } from 'vue'
import { formatJson, minifyJson } from './useFormat'
import { useAutoConvert } from './useAutoConvert'
import { useCopy } from '@/core/feedback/useCopy'
import { UiAlert, UiButton, UiCodeEditor, UiRadioGroup, UiToolbar } from '@/core/ui'

const { copyText } = useCopy()

type JsonMode = 'format' | 'minify'

const input = ref('{\n  "name": "CoveKit",\n  "tools": 8\n}')
const output = ref('')
const errorMsg = ref('')
const mode = ref<JsonMode>('format')

const modeOptions = [
  { value: 'format', label: '格式化' },
  { value: 'minify', label: '压缩' },
]

/** 按当前模式执行转换；空输入清空结果，不报错 */
function run() {
  const text = input.value
  if (!text.trim()) {
    output.value = ''
    errorMsg.value = ''
    return
  }
  // 缩进固定 2 空格：输出缩进是格式化结果的一部分，但为它长期占一个配置项不划算，
  // 2 空格在 JSON 场景是压倒性默认；真需要 4 空格 / Tab 时再考虑做进面板工具条
  const r = mode.value === 'format' ? formatJson(text, 2) : minifyJson(text)
  errorMsg.value = r.error
    ? `${r.error.message}（第 ${r.error.line} 行，第 ${r.error.col} 列）`
    : ''
  output.value = r.output
}

const { watchInput, runNow } = useAutoConvert(run)
watchInput(input)
// 模式切换立即重算，不等防抖
watch(mode, runNow)
// 挂载即有示例输入，立即出结果
runNow()

function clearAll() {
  input.value = ''
  output.value = ''
  errorMsg.value = ''
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[10px]">
    <UiToolbar class="shrink-0">
      <UiRadioGroup
        v-model="mode"
        :options="modeOptions"
        name="json-mode"
        variant="chips"
        size="sm"
        aria-label="JSON 转换模式"
      />
      <UiButton class="ml-auto" variant="ghost" @click="clearAll">清空</UiButton>
      <UiButton v-if="output" variant="ghost" @click="copyText(output, 'JSON 已复制')">
        复制结果
      </UiButton>
    </UiToolbar>

    <UiAlert v-if="errorMsg" tone="danger" class="shrink-0">{{ errorMsg }}</UiAlert>

    <div class="grid min-h-0 flex-1 grid-cols-2 gap-[12px]">
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输入 JSON</label>
        <UiCodeEditor
          v-model="input"
          class="min-h-0 flex-1"
          placeholder='输入 JSON，如 {"a": 1}'
          mode="minimal"
        />
      </div>
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输出</label>
        <UiCodeEditor :model-value="output" readonly language="json" />
      </div>
    </div>
  </div>
</template>
