<script setup lang="ts">
/**
 * 关于与更新：读取当前安装版本，提供发布说明入口并展示更新状态。
 *
 * 更新状态、重入保护、取消口径都在 `stores/update` 里，
 * 因此离开设置页再回来仍能看到「正在下载/待安装/上次失败原因」，句柄也不会随组件卸载丢掉。
 * 下载阶段可取消，安装阶段不可取消——原因由 store 给出并直接展示。
 */
import { computed, onMounted, ref } from 'vue'
import { getVersion } from '@tauri-apps/api/app'
import { isTauri } from '@tauri-apps/api/core'
import { version as previewVersion } from '../../../package.json'
import { RELEASES_URL } from '@/core/project'
import { ipc } from '@/core/ipc/ipc'
import { useUiStore } from '@/stores/ui'
import covekitIcon from '@/assets/covekit-icon-color.png'
import { useI18n } from 'vue-i18n'
import { UiButton } from '@/core/ui'
import { useUpdateStore } from '@/stores/update'

const { t } = useI18n()
const update = useUpdateStore()
const ui = useUiStore()
const currentVersion = ref(isTauri() ? '' : previewVersion)
const versionError = ref(false)

onMounted(async () => {
  if (!isTauri()) return
  try {
    currentVersion.value = await getVersion()
  } catch {
    versionError.value = true
  }
})

async function openRelease(current = false) {
  try {
    await ipc.openExternal(
      current ? `${RELEASES_URL}/tag/v${encodeURIComponent(currentVersion.value)}` : RELEASES_URL
    )
  } catch {
    ui.toast(t('settings.openLinkFailed'))
  }
}

onMounted(() => {
  // 进设置页刷新一次可用性：更新通道可能因为配置变化而变可用/不可用
  void update.ensureAvailability()
})

/** 进度展示：总量未知时退化成已下载字节数 */
const progress = computed(() => {
  if (update.phase !== 'downloading') return ''
  if (update.percent === null) return `${Math.round(update.downloaded / 1024)} KB`
  return `${update.percent}%`
})

const statusText = computed(() => {
  switch (update.phase) {
    case 'unsupported':
      return t('settings.updateUnsupported')
    case 'unavailable':
      return t('settings.updateUnavailable', { reason: update.unavailableReason })
    case 'checking':
      return t('settings.updateChecking')
    case 'latest':
      return t('settings.updateLatest')
    case 'available':
      return t('settings.updateAvailable', { version: update.version })
    case 'downloading':
      return t('settings.updateDownloading', { progress: progress.value })
    case 'ready':
      return t('settings.updateReady', { version: update.version })
    case 'installing':
      return t('settings.updateInstalling')
    case 'error':
      return t('settings.updateError', { message: update.errorMessage })
    default:
      return t('settings.updateIdle')
  }
})
</script>

<template>
  <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
    <h3 class="text-h2 font-bold dark:text-primary-dark">{{ t('settings.about') }}</h3>
    <div class="mt-md flex items-center gap-md">
      <img :src="covekitIcon" alt="" class="h-12 w-12 rounded-md" />
      <div>
        <p class="text-h2 font-bold dark:text-primary-dark">CoveKit</p>
        <p class="select-text text-body-sm text-secondary dark:text-secondary-dark">
          {{
            versionError
              ? t('settings.versionFailed')
              : t('settings.currentVersion', { version: currentVersion || '…' })
          }}
        </p>
      </div>
    </div>
    <div class="mt-sm flex flex-wrap gap-sm">
      <UiButton variant="ghost" size="sm" :disabled="!currentVersion" @click="openRelease(true)">{{
        t('settings.releaseNotes')
      }}</UiButton>
      <UiButton variant="ghost" size="sm" @click="openRelease()">{{
        t('settings.releaseHistory')
      }}</UiButton>
    </div>
    <div
      class="mt-md flex flex-wrap items-center justify-between gap-sm border-t border-border pt-md dark:border-border-dark"
    >
      <p class="text-body-sm text-text-muted dark:text-text-muted-dark">{{ statusText }}</p>
      <div class="flex items-center gap-sm">
        <UiButton v-if="update.canCancel" variant="ghost" @click="update.cancel()">
          {{ t('settings.cancelDownload') }}
        </UiButton>
        <UiButton v-if="update.phase === 'available'" @click="update.download()">
          {{ t('settings.downloadUpdate') }}
        </UiButton>
        <UiButton v-else-if="update.phase === 'ready'" @click="update.install()">
          {{ t('settings.installUpdate') }}
        </UiButton>
        <UiButton v-else :disabled="!update.canCheck" @click="update.checkNow()">
          {{ t('settings.checkUpdate') }}
        </UiButton>
      </div>
    </div>
    <div v-if="update.version && update.phase !== 'checking'" class="mt-md">
      <h4 class="text-body font-medium dark:text-primary-dark">
        {{ t('settings.newReleaseNotes', { version: update.version }) }}
      </h4>
      <p
        class="select-text mt-sm whitespace-pre-wrap break-words text-body-sm text-secondary dark:text-secondary-dark"
      >
        {{ update.releaseNotes || t('settings.noReleaseNotes') }}
      </p>
    </div>
    <p
      v-if="update.phase === 'installing'"
      class="mt-xs text-body-sm text-text-muted dark:text-text-muted-dark"
    >
      {{ update.installNotCancellableReason }}
    </p>
  </section>
</template>
