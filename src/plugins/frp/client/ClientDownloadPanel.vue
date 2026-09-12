<script setup lang="ts">
/**
 * ClientDownloadPanel · 一键下载官方版本（客户端管理的添加入口之一）
 * 复用 useFrpBinary 的下载与版本列表；进度走 frp://download 事件。
 * 下载成功由父级刷新清单（Rust 侧会把新装的版本自动登记为客户端）。
 */
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiAlert, UiButton, UiSelect } from '@/core/ui'
import { useFrpBinary } from '../binary/useFrpBinary'
import DownloadProgress from '../binary/DownloadProgress.vue'

const emit = defineEmits<{
  /** 安装完成（带上版本号，父级用于提示与刷新清单） */
  installed: [version: string]
}>()

const { t } = useI18n()
const binary = useFrpBinary()

/** 选中的上游版本 */
const version = ref('')

/** 版本下拉选项（带发布日期的简写） */
const options = computed(() =>
  binary.versions.value.map((item) => ({
    value: item.version,
    label: `${item.version} · ${item.publishedAt.slice(0, 10)}`,
  }))
)

onMounted(async () => {
  await binary.loadVersions(10)
  // 默认选中最新版：绝大多数场景就是装最新
  const latest = binary.versions.value[0]
  if (latest) version.value = latest.version
})

/** 下载选中版本；成功后通知父级刷新清单 */
async function onDownload() {
  if (version.value === '') return
  const result = await binary.download(version.value)
  if (result?.ok) emit('installed', version.value)
}
</script>

<template>
  <div class="flex flex-col gap-[8px]">
    <!-- 与「引用已有 frpc」平行的一句说明，免得这一栏看起来没有来由 -->
    <p class="text-caption text-text-muted dark:text-text-muted-dark">
      {{ t('frp.clientDownloadHint') }}
    </p>
    <div class="flex items-center gap-[8px]">
      <UiSelect
        v-model="version"
        class="min-w-0 flex-1"
        size="sm"
        :options="options"
        :placeholder="
          binary.loadingVersions.value ? t('frp.binaryLoadingVersions') : t('frp.binaryNoVersion')
        "
        :disabled="binary.downloading.value || options.length === 0"
      />
      <UiButton
        size="sm"
        :disabled="binary.downloading.value || version === ''"
        @click="onDownload"
      >
        {{ t('frp.binaryDownloadButton') }}
      </UiButton>
    </div>

    <DownloadProgress
      v-if="binary.downloading.value"
      :progress="binary.progress.value"
      :speed="binary.speed.value"
    />

    <!-- 失败原因就地可见，不静默（版本列表失败给出镜像前缀等可操作提示） -->
    <UiAlert v-if="binary.versionsError.value !== ''" tone="warning">
      {{ t('frp.binaryVersionsFailed', { message: binary.versionsError.value }) }}
    </UiAlert>
    <UiAlert v-if="binary.downloadError.value !== ''" tone="danger">
      {{ t('frp.binaryDownloadFailed', { message: binary.downloadError.value }) }}
    </UiAlert>
  </div>
</template>
