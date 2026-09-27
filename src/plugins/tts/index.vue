<script setup lang="ts">
/**
 * 文字转语音 · 主界面
 * 文本输入 → 选语音（中文优先）→ 语速/音调调节 → 合成 mp3 → 播放/下载。
 * 后端走微软 Edge TTS 免费服务（无需 API key）。
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useUiStore } from '@/stores/ui'
import {
  UiButton,
  UiEmptyState,
  UiField,
  UiSlider,
  UiSelect as Select,
  UiTextarea,
  UiToolbar,
} from '@/core/ui'
import AppIcon from '@/features/ui/AppIcon.vue'
import { ipc } from './ipc'
import type { TtsVoice } from './contracts'

const ui = useUiStore()

/** 输入文本 */
const text = ref('你好，欢迎使用 CoveKit 文字转语音工具。这是一段中文语音测试。')
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
const audioElement = ref<HTMLAudioElement | null>(null)
let disposed = false
let generation = 0
let activeJob: string | undefined

async function cancelJob(jobId: string) {
  try {
    await ipc.ttsCancel(jobId)
  } catch (error) {
    ui.toast(`取消语音合成失败：${error}`)
  }
}

function cancelActiveJob() {
  const job = activeJob
  activeJob = undefined
  if (job) void cancelJob(job)
}

/** 移除 DOM 本身不会立即停止媒体；清空和关闭时主动释放播放与解码缓冲。 */
function releaseAudio() {
  const audio = audioElement.value
  if (!audio) return
  audio.pause()
  audio.removeAttribute('src')
  audio.load()
}

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
  if (disposed || generating.value) return
  if (!text.value.trim()) {
    ui.toast('请输入要合成的文字')
    return
  }
  if (text.value.length > 2000) {
    ui.toast('文本过长（最多 2000 字）')
    return
  }
  generating.value = true
  const request = ++generation
  const input = { text: text.value, voice: voiceName.value, rate: rate.value, pitch: pitch.value }
  let jobId: string | undefined
  try {
    jobId = await ipc.ttsPrepare()
    if (disposed || request !== generation) {
      await cancelJob(jobId)
      return
    }
    activeJob = jobId
    const r = await ipc.ttsSynthesize({
      ...input,
      jobId,
    })
    if (disposed || request !== generation) {
      if (r.ok && r.filePath) {
        try {
          await ipc.ttsDiscard(jobId)
        } catch (error) {
          ui.toast(`清理过期音频失败：${error}`)
        }
      }
      return
    }
    if (r.ok && r.filePath) {
      audioFile.value = r.filePath
      audioUrl.value = convertFileSrc(r.filePath)
      lastBytes.value = r.bytes
      ui.toast(`合成成功（${(r.bytes / 1024).toFixed(1)} KB）`)
    } else {
      ui.toast(`合成失败：${r.error ?? '未知错误'}`)
    }
  } catch (e) {
    // IPC 本身失败时后端可能尚未领用登记，仍按归属回收，不留下空请求。
    if (jobId && activeJob === jobId) await cancelJob(jobId)
    if (!disposed && request === generation) ui.toast(`合成失败：${e}`)
  } finally {
    if (activeJob === jobId) activeJob = undefined
    if (!disposed && request === generation) generating.value = false
  }
}

function reset() {
  generation++
  cancelActiveJob()
  generating.value = false
  releaseAudio()
  text.value = ''
  audioUrl.value = ''
  audioFile.value = ''
  lastBytes.value = 0
}

onMounted(async () => {
  try {
    const result = await ipc.ttsVoices()
    if (!disposed) voices.value = result
  } catch (e) {
    if (!disposed) ui.toast(`语音列表加载失败：${e}`)
  }
})

onBeforeUnmount(() => {
  disposed = true
  generation++
  cancelActiveJob()
  releaseAudio()
})
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 顶栏 -->
    <UiToolbar bordered title="文字转语音">
      <span class="font-mono text-caption text-text-muted dark:text-text-muted-dark">
        微软 Edge TTS · 免费中文语音
      </span>
    </UiToolbar>

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
            <UiSlider v-model="rate" :min="-50" :max="50" :step="5" />
          </UiField>
          <UiField :label="`音调（${pitch > 0 ? '+' : ''}${pitch}Hz）`">
            <UiSlider v-model="pitch" :min="-50" :max="50" :step="5" />
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
          <audio ref="audioElement" :src="audioUrl" controls class="w-full" />
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
