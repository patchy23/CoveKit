<script setup lang="ts">
/**
 * XML 面板 · 格式化/压缩/校验（格式转换工具子页签；与 JSON 面板同款分栏布局）
 * 免按钮交互（2026-09-18）：输入停顿 300ms 自动转换，模式切换立即重算。
 */
import { ref, watch, onScopeDispose } from 'vue'
import { createXmlFormatter } from '@/core/format/asyncXml'
import { useAutoConvert } from './useAutoConvert'
import { useCopy } from '@/core/feedback/useCopy'
import { UiAlert, UiButton, UiCodeEditor, UiRadioGroup, UiToolbar } from '@/core/ui'

const { copyText } = useCopy()

type XmlMode = 'format' | 'minify'

const input = ref(
  '<?xml version="1.0" encoding="UTF-8"?>\n<config>\n  <app name="CoveKit">\n    <version>0.1.0</version>\n  </app>\n</config>'
)
const output = ref('')
const error = ref('')
const mode = ref<XmlMode>('format')
const formatter = createXmlFormatter()
const pending = ref(false)
let revision = 0
watch(
  [input, mode],
  () => {
    revision++
    formatter.cancel()
    pending.value = false
  },
  { flush: 'sync' }
)
onScopeDispose(() => {
  revision++
  formatter.destroy()
})

const modeOptions = [
  { value: 'format', label: '格式化' },
  { value: 'minify', label: '压缩' },
]

/** 按当前模式执行转换；空输入清空结果，不报错 */
async function run() {
  const version = ++revision
  error.value = ''
  const text = input.value
  if (!text.trim()) {
    output.value = ''
    return
  }
  try {
    const result = formatter.run(text, 2, mode.value)
    pending.value = result instanceof Promise
    const r = result instanceof Promise ? await result : result
    if (version !== revision) return
    if (r.ok) {
      output.value = r.output ?? ''
      if (r.loose) {
        error.value = '宽松模式：命名空间或结构未通过严格校验，已按标签缩进格式化'
      }
    } else {
      output.value = ''
      error.value = r.error ?? '格式化失败'
    }
  } catch (cause) {
    if (version === revision) error.value = '转换失败：' + String(cause)
  } finally {
    if (version === revision) pending.value = false
  }
}

const { watchInput, runNow } = useAutoConvert(run)
watchInput(input)
// 模式切换立即重算，不等防抖
watch(mode, runNow)
// 挂载即有示例输入，立即出结果
runNow()
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[10px]">
    <UiToolbar class="shrink-0">
      <UiRadioGroup
        v-model="mode"
        :options="modeOptions"
        name="xml-mode"
        variant="chips"
        size="sm"
        aria-label="XML 转换模式"
      />
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
    <span v-if="pending" class="text-body-sm text-text-muted">正在转换…</span>

    <div class="grid min-h-0 flex-1 grid-cols-2 gap-[12px]">
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输入 XML</label>
        <UiCodeEditor v-model="input" class="min-h-0 flex-1" mode="minimal" />
      </div>
      <div class="flex min-h-0 flex-col">
        <label class="mb-[6px] shrink-0 field-label">输出</label>
        <UiCodeEditor :model-value="output" readonly language="xml" />
      </div>
    </div>
  </div>
</template>
