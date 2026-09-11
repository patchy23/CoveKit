<script setup lang="ts">
/**
 * BinarySetupCard · frpc 未配置时的引导卡
 * 两条路径：一键下载（列出上游版本 + 进度）与指定已有 frpc（文件选择 + 写工具设置）。
 * 下载成功后通知父级重新探测；失败原因就地可见，不静默。
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { UiAlert, UiButton, UiProgress, UiSelect, UiSpinner } from '@/core/ui'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'
import type { FrpBinaryInfo } from '../contracts'
import { useFrpBinary } from './useFrpBinary'

const props = defineProps<{
  /** 当前探测结果（null = 尚未探测） */
  detected: FrpBinaryInfo | null
}>()
const emit = defineEmits<{ changed: [] }>()

const { t } = useI18n()
const ui = useUiStore()
const settings = useSettingsStore()
const binary = useFrpBinary()

/** 当前选中的上游版本（版本列表加载完成后默认取最新） */
const selectedVersion = ref('')
watch(
  () => binary.versions.value,
  (list) => {
    if (selectedVersion.value === '' && list.length > 0) selectedVersion.value = list[0].version
  },
  { immediate: true }
)

/** 版本下拉选项（带发布日期的简写） */
const versionOptions = computed(() =>
  binary.versions.value.map((item) => ({
    value: item.version,
    label: `${item.version} · ${item.publishedAt.slice(0, 10)}`,
  }))
)

/** 下载进度文本（服务端未给总长度时只显示已接收量） */
const progressText = computed(() => {
  const current = binary.progress.value
  if (!current) return t('frp.binaryPreparing')
  const received = current.received ?? 0
  if (current.total === undefined || current.total <= 0) {
    return t('frp.binaryDownloadingUnknown', { received: formatBytes(received) })
  }
  return t('frp.binaryDownloading', {
    received: formatBytes(received),
    total: formatBytes(current.total),
  })
})

/** 字节数格式化（本地实现，避免为一个显示函数引入依赖） */
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

/** 选择本地已有 frpc 可执行文件并写入工具设置 */
async function pickExisting() {
  const selected = await dialogOpen({
    multiple: false,
    directory: false,
    title: t('frp.binaryPickTitle'),
  })
  if (typeof selected !== 'string') return
  await settings.setToolSetting('frp', 'frpcPath', selected)
  const result = await binary.detect(selected)
  if (result?.ok) {
    ui.toast(t('frp.binaryPickOk'))
    emit('changed')
    return
  }
  ui.toast(t('frp.binaryPickFailed', { message: result?.error ?? t('frp.unknownError') }))
}

/** 下载选中版本；成功后写回最新路径（设置项留空，让探测继续生效） */
async function downloadSelected() {
  const version = selectedVersion.value
  if (version === '') {
    ui.toast(t('frp.binaryNoVersion'))
    return
  }
  const result = await binary.download(version)
  if (result?.ok) {
    ui.toast(t('frp.binaryDownloaded', { version }))
    emit('changed')
    return
  }
  if (binary.downloadError.value !== '') {
    ui.toast(t('frp.binaryDownloadFailed', { message: binary.downloadError.value }))
  }
}

onMounted(() => {
  void binary.loadVersions()
})
</script>

<template>
  <div class="flex h-full min-h-0 flex-col items-center justify-center gap-[14px] px-[24px]">
    <div
      class="w-full max-w-[520px] rounded-lg border border-border p-[18px] dark:border-border-dark"
    >
      <h3 class="text-card-title font-bold dark:text-primary-dark">
        {{ t('frp.binaryTitle') }}
      </h3>
      <p class="mt-[6px] text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ t('frp.binaryHint') }}
      </p>

      <!-- 一键下载 -->
      <div class="mt-[14px] flex flex-col gap-[8px]">
        <label class="text-body-sm font-medium dark:text-primary-dark">
          {{ t('frp.binaryDownloadLabel') }}
        </label>
        <div class="flex items-center gap-[8px]">
          <UiSelect
            v-model="selectedVersion"
            class="min-w-0 flex-1"
            :options="versionOptions"
            :placeholder="
              binary.loadingVersions.value
                ? t('frp.binaryLoadingVersions')
                : t('frp.binaryNoVersion')
            "
            :disabled="binary.downloading.value || versionOptions.length === 0"
          />
          <UiButton
            :disabled="binary.downloading.value || selectedVersion === ''"
            @click="downloadSelected"
          >
            {{ t('frp.binaryDownloadButton') }}
          </UiButton>
        </div>

        <!-- 下载进度（下载中才出现；总量未知时只留转圈与文本） -->
        <div v-if="binary.downloading.value" class="flex items-center gap-[10px]">
          <UiSpinner size="sm" />
          <UiProgress
            v-if="(binary.progress.value?.total ?? 0) > 0"
            class="min-w-0 flex-1"
            size="sm"
            :value="binary.progress.value?.received ?? 0"
            :max="binary.progress.value?.total ?? 1"
            show-value
          />
          <span v-else class="text-caption text-secondary dark:text-secondary-dark">
            {{ progressText }}
          </span>
        </div>

        <!-- 版本列表失败：给出可操作提示（手填路径与镜像前缀） -->
        <UiAlert v-if="binary.versionsError.value !== ''" tone="warning">
          {{ t('frp.binaryVersionsFailed', { message: binary.versionsError.value }) }}
        </UiAlert>
        <UiAlert v-if="binary.downloadError.value !== ''" tone="danger">
          {{ t('frp.binaryDownloadFailed', { message: binary.downloadError.value }) }}
        </UiAlert>
      </div>

      <div class="my-[14px] border-t border-border dark:border-border-dark" />

      <!-- 指定已有 frpc -->
      <div class="flex items-center justify-between gap-[10px]">
        <div class="min-w-0">
          <p class="text-body-sm font-medium dark:text-primary-dark">
            {{ t('frp.binaryPickLabel') }}
          </p>
          <p
            v-if="props.detected?.path"
            class="mt-[2px] truncate font-mono text-caption text-text-muted dark:text-text-muted-dark"
            :title="props.detected.path"
          >
            {{ props.detected.path }}
          </p>
        </div>
        <UiButton size="sm" :disabled="binary.downloading.value" @click="pickExisting">
          {{ t('frp.binaryPickButton') }}
        </UiButton>
      </div>

      <p class="mt-[10px] text-caption text-text-muted dark:text-text-muted-dark">
        {{ t('frp.binaryFooterHint') }}
      </p>
    </div>
  </div>
</template>
