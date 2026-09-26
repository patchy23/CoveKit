<script setup lang="ts">
/**
 * Base64 面板 · 文本 ⇄ Base64 双向编解码（格式转换工具子页签；UTF-8 安全）
 * 布局与 JSON/XML 面板一致：输入/输出左右分栏。
 * 免按钮交互（2026-09-18）：输入停顿 300ms 自动转换，编码/解码方向切换立即重算
 * （方向用显式切换而非内容嗅探——与 DevToys 一致，避免合法 Base64 文本被误判方向）。
 */
import { onScopeDispose, ref, watch } from 'vue'
import { createBase64Task } from './base64Task'
import { useAutoConvert } from './useAutoConvert'
import { useCopy } from '@/core/feedback/useCopy'
import { UiAlert, UiButton, UiCodeEditor, UiRadioGroup, UiToolbar } from '@/core/ui'

const { copyText } = useCopy()

type Direction = 'encode' | 'decode'

const input = ref('')
const output = ref('')
const error = ref('')
const direction = ref<Direction>('encode')
const task = createBase64Task()
const pending = ref(false)
let revision = 0
watch(
  [input, direction],
  () => {
    revision++
    task.cancel()
    pending.value = false
  },
  { flush: 'sync' }
)
onScopeDispose(() => {
  revision++
  task.destroy()
})

const directionOptions = [
  { value: 'encode', label: '编码' },
  { value: 'decode', label: '解码' },
]

/** 按当前方向执行转换；空输入清空结果，不报错 */
async function run() {
  const request = ++revision
  const text = input.value
  if (!text) {
    output.value = ''
    error.value = ''
    return
  }
  pending.value = true
  try {
    const work = task.run(text, direction.value)
    const r = work instanceof Promise ? await work : work
    if (request !== revision) return
    error.value = r.ok ? '' : (r.error ?? (direction.value === 'encode' ? '编码失败' : '解码失败'))
    output.value = r.output
  } catch (reason) {
    if (request === revision) error.value = `转换失败：${String(reason)}`
  } finally {
    if (request === revision) pending.value = false
  }
}

const { watchInput, runNow } = useAutoConvert(run)
watchInput(input)
// 方向切换立即重算，不等防抖
watch(direction, runNow)

function clearAll() {
  input.value = ''
  output.value = ''
  error.value = ''
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[10px]">
    <UiToolbar class="shrink-0">
      <UiRadioGroup
        v-model="direction"
        :options="directionOptions"
        name="base64-direction"
        variant="chips"
        size="sm"
        aria-label="Base64 转换方向"
      />
      <UiButton class="ml-auto" variant="ghost" @click="clearAll">清空</UiButton>
      <UiButton v-if="output" variant="ghost" @click="copyText(output, '已复制结果')">
        复制结果
      </UiButton>
      <span class="text-body-sm text-text-muted dark:text-text-muted-dark">支持中文（UTF-8）</span>
    </UiToolbar>

    <UiAlert v-if="error" tone="danger" class="shrink-0">{{ error }}</UiAlert>
    <span v-else-if="pending" class="text-caption text-text-muted">正在转换…</span>

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
