<script setup lang="ts">
/**
 * DownloadProgress · 下载进度展示（字节进度条 + 已下载/总大小 · 速率）
 * 引导卡与客户端管理弹窗共用，避免两处各写一套进度文案。
 * 速率由调用方用滑动窗口估算（见 binary/downloadSpeed.ts）：累计均值在链路变慢时仍显示虚高，
 * 且下载卡住时只会缓慢下降、看不出卡死。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiProgress, UiSpinner } from '@/core/ui'
import { formatBytes } from '@/core/format/bytes'
import type { FrpDownloadPayload } from '../contracts'

const props = defineProps<{
  /** 最新进度事件（null = 尚未开始） */
  progress: FrpDownloadPayload | null
  /** 当前速率（字节/秒；null = 样本不足，不显示，避免闪一个假数字） */
  speed: number | null
}>()

const { t } = useI18n()

/** 是否已知总量（未知时省略进度条，只显示已接收量与速率） */
const total = computed(() => props.progress?.total ?? 0)

/**
 * 阶段与字节数文案。校验与解压阶段字节数不再增长，
 * 继续显示停住的数字会让人以为卡死，因此改为说明当前在做什么。
 */
const text = computed(() => {
  const current = props.progress
  if (current === null) return t('frp.binaryPreparing')
  if (current.phase === 'verify') return t('frp.binaryVerifying')
  if (current.phase === 'extract') return t('frp.binaryExtracting')
  const received = formatBytes(current.received ?? 0)
  const bytes = current.total ?? 0
  if (bytes <= 0) return t('frp.binaryDownloadingUnknown', { received })
  return t('frp.binaryDownloading', { received, total: formatBytes(bytes) })
})

/** 速率文案（样本不足时为空串） */
const speedText = computed(() =>
  props.speed === null ? '' : t('frp.binarySpeed', { speed: `${formatBytes(props.speed)}/s` })
)
</script>

<template>
  <div class="flex flex-col gap-[6px]">
    <UiProgress v-if="total > 0" size="sm" :value="props.progress?.received ?? 0" :max="total" />
    <div class="flex items-center gap-[8px] text-caption">
      <UiSpinner size="sm" />
      <span class="text-secondary dark:text-secondary-dark">{{ text }}</span>
      <span v-if="speedText !== ''" class="text-text-muted dark:text-text-muted-dark">
        · {{ speedText }}
      </span>
    </div>
  </div>
</template>
