<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { UiButton } from '@/core/ui'

const { t } = useI18n()
const state = ref<'idle' | 'checking' | 'latest' | 'available' | 'installing' | 'error'>('idle')
const update = ref<Update | null>(null)
const downloaded = ref(0)
const total = ref<number | undefined>()
const error = ref('')
const updaterSupported = '__TAURI_INTERNALS__' in window

const progress = computed(() => {
  if (!total.value) return `${Math.round(downloaded.value / 1024)} KB`
  return `${Math.min(100, Math.round((downloaded.value / total.value) * 100))}%`
})
const statusText = computed(() => {
  if (state.value === 'checking') return t('settings.updateChecking')
  if (state.value === 'latest') return t('settings.updateLatest')
  if (state.value === 'available')
    return t('settings.updateAvailable', { version: update.value?.version })
  if (state.value === 'installing')
    return t('settings.updateInstalling', { progress: progress.value })
  if (state.value === 'error') return t('settings.updateError', { message: error.value })
  return updaterSupported ? t('settings.updateIdle') : t('settings.updateUnsupported')
})

async function checkForUpdate() {
  state.value = 'checking'
  error.value = ''
  try {
    update.value = await check()
    state.value = update.value ? 'available' : 'latest'
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason)
    state.value = 'error'
  }
}

async function installUpdate() {
  if (!update.value) return
  state.value = 'installing'
  downloaded.value = 0
  try {
    await update.value.downloadAndInstall((event) => {
      if (event.event === 'Started') total.value = event.data.contentLength
      if (event.event === 'Progress') downloaded.value += event.data.chunkLength
    })
    await relaunch()
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason)
    state.value = 'error'
  }
}

onBeforeUnmount(() => void update.value?.close())
</script>

<template>
  <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
    <h3 class="text-h2 font-bold dark:text-primary-dark">{{ t('settings.update') }}</h3>
    <div class="mt-sm flex items-center justify-between gap-sm">
      <p class="text-body-sm text-text-muted dark:text-text-muted-dark">{{ statusText }}</p>
      <UiButton
        v-if="state !== 'available'"
        :disabled="state === 'checking' || state === 'installing' || !updaterSupported"
        @click="checkForUpdate"
      >
        {{ t('settings.checkUpdate') }}
      </UiButton>
      <UiButton v-else @click="installUpdate">{{ t('settings.installUpdate') }}</UiButton>
    </div>
  </section>
</template>
