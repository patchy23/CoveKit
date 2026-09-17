<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * RuntimeLogPanel · 运行时日志
 * 等级着色、关键字过滤、自动滚动（用户上滚即暂停，符合终端直觉）、一键清空。
 * 日志是明文数据，使用等宽字体；行内不换行，横向滚动。
 */
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiButton, UiIcon, UiIconButton, UiInput } from '@/core/ui'
import { logLevelClass, type FrpLogLine } from './frpStatus'

const props = defineProps<{
  /** 日志行（已按上限裁剪） */
  lines: FrpLogLine[]
  /** 是否正在运行（决定空状态文案） */
  running: boolean
}>()
const emit = defineEmits<{ clear: [] }>()

const { t } = useI18n()

/** 关键字过滤（大小写不敏感，匹配日志正文） */
const keyword = ref('')
/** 自动滚动开关（用户手动上滚自动置 false） */
const autoScroll = ref(true)
/** 滚动容器 */
const scroller = ref<HTMLElement | null>(null)

/** 过滤后的日志行 */
const visible = computed(() => {
  const needle = keyword.value.trim().toLowerCase()
  if (needle === '') return props.lines
  return props.lines.filter((item) => item.line.toLowerCase().includes(needle))
})

/** 时间戳 → 时分秒（同日日志只看时间，跨天信息在 frpc 自身日志里） */
function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString('en-GB', { hour12: false })
}

/** 滚到底部（仅在自动滚动开启时执行） */
function scrollToBottom(): void {
  const element = scroller.value
  if (element === null || !autoScroll.value) return
  element.scrollTop = element.scrollHeight
}

/** 用户滚动：贴底视为开启自动滚动，上滚即暂停 */
function onScroll(): void {
  const element = scroller.value
  if (element === null) return
  const atBottom = element.scrollHeight - element.scrollTop - element.clientHeight < 24
  autoScroll.value = atBottom
}

watch(
  () => props.lines.length,
  () => {
    void nextTick(scrollToBottom)
  }
)

watch(
  () => props.lines,
  () => {
    void nextTick(scrollToBottom)
  }
)
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 工具条：过滤 + 自动滚动 + 清空 -->
    <div
      class="flex shrink-0 items-center gap-[8px] border-b border-border px-[10px] py-[6px] dark:border-border-dark"
    >
      <UiInput
        v-model="keyword"
        size="sm"
        class="min-w-0 flex-1"
        :placeholder="t('frp.logFilterPlaceholder')"
      />
      <UiButton
        size="xs"
        :variant="autoScroll ? 'primary' : 'secondary'"
        @click="autoScroll = !autoScroll"
      >
        {{ t('frp.logAutoScroll') }}
      </UiButton>
      <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
        {{ t('frp.logLineCount', { count: visible.length }) }}
      </span>
      <UiIconButton :label="t('frp.logClear')" size="xs" @click="emit('clear')">
        <UiIcon name="trash" :size="14" />
      </UiIconButton>
    </div>

    <!-- 日志区 -->
    <UiScrollArea as-child axis="both">
      <div
        ref="scroller"
        class="min-h-0 flex-1 bg-neutral px-[10px] py-[6px] dark:bg-neutral-dark"
        @scroll="onScroll"
      >
        <p
          v-if="visible.length === 0"
          class="flex items-center gap-[6px] py-[8px] text-body-sm text-text-muted dark:text-text-muted-dark"
        >
          <UiIcon name="info" :size="14" />
          {{ props.running ? t('frp.logEmptyRunning') : t('frp.logEmptyStopped') }}
        </p>
        <div
          v-for="(item, index) in visible"
          :key="`${item.ts}-${index}`"
          class="flex items-start gap-[8px] whitespace-pre font-mono text-body-sm leading-[1.5]"
        >
          <span class="shrink-0 text-text-muted dark:text-text-muted-dark">{{
            formatTime(item.ts)
          }}</span>
          <span class="min-w-0 flex-1 break-all" :class="logLevelClass(item.level)">{{
            item.line
          }}</span>
        </div>
      </div>
    </UiScrollArea>
  </div>
</template>
