<script setup lang="ts">
/**
 * 文字转语音 · 主界面
 * 文本输入 → 选语音（中文优先）→ 语速/音调调节 → 合成 mp3 → 播放/下载。
 * 后端走微软 Edge TTS 免费服务（无需 API key）。
 */
import { computed, onMounted, ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useUiStore } from '@/stores/ui'
import { UiButton, UiEmptyState, UiField, UiRange, UiSelect as Select, UiTextarea } from '@/core/ui'
import AppIcon from '@/features/ui/AppIcon.vue'
import { ipc } from './ipc'
import type { TtsVoice } from './contracts'

const ui = useUiStore()

/** 输入文本 */
const text = ref('你好，欢迎使用 Hekara 文字转语音工具。这是一段中文语音测试。')
/** 语音列表与选中项 */
const voices = ref<TtsVoice[]>([])
const voiceName = ref('zh-CN-XiaoxiaoNeural')
/** 语速（-50 ~ +50%）与音调（-50 ~ +50Hz） */
const rate = ref(0)
const pitch = ref(0)

/** 合成状态与结果 */
const generating = ref(false)
const audioUrl = ref<string>('')
const audioFile = ref('')
const lastBytes = ref(0)

/** 按语言分组：中文组在前 */
const voiceOptions = computed(() => {
  const zh = voices.value.filter((v) => v.lang.startsWith('zh'))
  const other = voices.value.filter((v) => !v.lang.startsWith('zh'))
  return [
    ...zh.map((v) => ({ value: v.name, label: v.label })),
    ...other.map((v) => ({ value: v.name, label: v.label })),
  ]
})

async function generate() {
  if (!text.value.trim()) {
    ui.toast('请输入要合成的文字')
    return
  }
  if (text.value.length > 2000) {
    ui.toast('文本过长（最多 2000 字）')
    return
  }
  generating.value = true
  try {
    const r = await ipc.ttsSynthesize({
      text: text.value,
      voice: voiceName.value,
      rate: rate.value,
      pitch: pitch.value,
    })
    if (r.ok && r.filePath) {
      audioFile.value = r.filePath
      audioUrl.value = convertFileSrc(r.filePath)
      lastBytes.value = r.bytes
      ui.toast(`合成成功（${(r.bytes / 1024).toFixed(1)} KB）`)
    } else {
      ui.toast(`合成失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    ui.toast(`合成失败：${e}`)
  } finally {
    generating.value = false
  }
}

function reset() {
  text.value = ''
  audioUrl.value = ''
  audioFile.value = ''
}

onMounted(async () => {
  try {
    voices.value = await ipc.ttsVoices()
  } catch (e) {
    ui.toast(`语音列表加载失败：${e}`)
  }
})
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 顶栏 -->
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">文字转语音</span>
      <span class="font-mono text-caption text-text-muted dark:text-text-muted-dark">
        微软 Edge TTS · 免费中文语音
      </span>
    </div>

    <div class="grid min-h-0 flex-1 grid-cols-2 gap-[14px] p-[14px]">
      <!-- 左：输入与参数 -->
      <div class="flex min-h-0 flex-col gap-[12px]">
        <UiTextarea
          v-model="text"
          class="min-h-0 flex-1 font-sans !leading-relaxed"
          resize="none"
          placeholder="输入要转成语音的文字（最多 2000 字）…"
          spellcheck="false"
        />
        <UiField label="音色" class="max-w-[320px]">
          <Select v-model="voiceName" :options="voiceOptions" />
        </UiField>
        <div class="grid grid-cols-2 gap-[14px]">
          <UiField :label="`语速（${rate > 0 ? '+' : ''}${rate}%）`">
            <UiRange v-model="rate" :min="-50" :max="50" :step="5" />
          </UiField>
          <UiField :label="`音调（${pitch > 0 ? '+' : ''}${pitch}Hz）`">
            <UiRange v-model="pitch" :min="-50" :max="50" :step="5" />
          </UiField>
        </div>
        <div class="flex gap-[10px]">
          <UiButton variant="primary" class="flex-1" :loading="generating" @click="generate">
            {{ generating ? '合成中…' : '合成语音' }}
          </UiButton>
          <UiButton variant="ghost" @click="reset">清空</UiButton>
        </div>
      </div>

      <!-- 右：结果 -->
      <div
        class="flex min-h-0 flex-col items-center justify-center gap-[12px] rounded-lg border border-border bg-surface p-[16px] dark:border-border-dark dark:bg-surface-dark"
      >
        <template v-if="audioUrl">
          <audio :src="audioUrl" controls class="w-full" />
          <p class="text-caption text-text-muted dark:text-text-muted-dark">
            已生成 {{ (lastBytes / 1024).toFixed(1) }} KB
          </p>
          <UiButton as="a" :href="audioUrl" :download="`tts-${Date.now()}.mp3`">
            下载 MP3
          </UiButton>
        </template>
        <UiEmptyState
          v-else
          title="等待合成语音"
          description="输入文字后点击「合成语音」，生成结果将在此播放。"
        >
          <template #icon><AppIcon name="tts" :size="24" /></template>
        </UiEmptyState>
      </div>
    </div>
  </div>
</template>
