<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/**
 * StorageRecoveryOverlay · 存储恢复页（框架级，覆盖整个窗口）
 *
 * 触发条件：配置的存储盘不可用（未插盘/只读/路径失效）或上次启动的根迁移失败。
 * 语义（任务书 T01）：不得静默退回默认目录新建一套空环境；磁盘重新出现也不会自动换根，
 * 必须由用户显式选择动作，且三个动作都**需要重启**才生效。
 * 可选动作：重试（重新探测配置盘）/ 选择新数据环境（登记迁移计划）/ 改用默认数据环境。
 */
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { relaunch } from '@tauri-apps/plugin-process'
import { UiButton, UiModal } from '@/core/ui'
import { ipc } from '@/core/ipc/ipc'
import type { StorageRecovery } from '@/core/ipc/contracts'
import { useUiStore } from '@/stores/ui'
import AppIcon from '@/features/ui/AppIcon.vue'

const { t } = useI18n()
const ui = useUiStore()

/** 当前恢复状态（null = 正常，不显示任何界面） */
const state = ref<StorageRecovery | null>(null)
/** 动作进行中（按钮禁用） */
const busy = ref(false)
/** 失败原因（恢复动作被拒绝时展示） */
const error = ref('')
/** 改用默认环境确认弹窗（破坏性：下次启动会打开另一套环境） */
const defaultConfirmVisible = ref(false)

/** 读取恢复状态（挂载时一次；恢复页本身就是启动期快照） */
async function load() {
  try {
    state.value = await ipc.storageRecoveryStatus()
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason)
  }
}

onMounted(load)

/** 执行恢复动作；成功后按后端返回的「需重启」提示用户重启 */
async function act(action: 'retry' | 'use-default' | 'choose', target?: string) {
  busy.value = true
  error.value = ''
  defaultConfirmVisible.value = false
  try {
    const result = await ipc.storageRecoveryAction(action, target)
    ui.toast(result.message)
    if (result.restartRequired) {
      // 动作改的是「下次启动」的解析结果，本进程不做热切换；刷新状态以反映已清除的恢复态
      state.value = await ipc.storageRecoveryStatus()
    }
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason)
    ui.toast(error.value)
  } finally {
    busy.value = false
  }
}

/** 重试：重新探测配置盘（可用则提示重启；仍不可用由后端回报原因） */
function retry() {
  void act('retry')
}

/** 选择新数据环境：目录选择 → 登记迁移计划（重启后复制并切换） */
async function chooseNew() {
  const selected = await dialogOpen({
    directory: true,
    multiple: false,
    defaultPath: state.value?.configuredRoot,
  })
  if (typeof selected !== 'string') return
  await act('choose', selected)
}

/** 立即重启 */
async function restartNow() {
  await relaunch()
}
</script>

<template>
  <div
    v-if="state"
    class="fixed inset-0 z-50 flex items-center justify-center bg-primary/40 p-[24px] backdrop-blur-sm"
  >
    <div
      class="flex w-full max-w-[560px] flex-col gap-sm rounded-lg border border-border bg-surface p-[20px] shadow-lg dark:border-border-dark dark:bg-surface-dark"
    >
      <h2 class="flex items-center gap-[8px] text-h1 font-bold dark:text-primary-dark">
        <AppIcon name="database" :size="18" class="text-danger-strong dark:text-danger-dark" />
        {{ t('storageRecovery.title') }}
      </h2>
      <p class="text-body text-secondary dark:text-secondary-dark">{{ state.detail }}</p>

      <div
        class="flex flex-col gap-[6px] rounded-sm border border-border p-[10px] dark:border-border-dark"
      >
        <div class="flex items-baseline gap-[8px]">
          <span class="w-[112px] shrink-0 text-body-sm text-text-muted dark:text-text-muted-dark">
            {{ t('storageRecovery.configuredRoot') }}
          </span>
          <UiTooltip :content="state.configuredRoot">
            <code class="min-w-0 flex-1 truncate font-mono text-body-sm dark:text-primary-dark">{{
              state.configuredRoot
            }}</code>
          </UiTooltip>
        </div>
        <div class="flex items-baseline gap-[8px]">
          <span class="w-[112px] shrink-0 text-body-sm text-text-muted dark:text-text-muted-dark">
            {{ t('storageRecovery.activeRoot') }}
          </span>
          <UiTooltip :content="state.activeRoot">
            <code class="min-w-0 flex-1 truncate font-mono text-body-sm dark:text-primary-dark">{{
              state.activeRoot
            }}</code>
          </UiTooltip>
        </div>
        <div v-if="state.planId" class="flex items-baseline gap-[8px]">
          <span class="w-[112px] shrink-0 text-body-sm text-text-muted dark:text-text-muted-dark">
            {{ t('storageRecovery.planId') }}
          </span>
          <span class="min-w-0 flex-1 truncate text-body-sm dark:text-primary-dark">{{
            state.planId
          }}</span>
        </div>
      </div>

      <p class="text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ t('storageRecovery.note') }}
      </p>

      <p v-if="error" class="text-body-sm text-danger-strong dark:text-danger-dark">{{ error }}</p>

      <div class="mt-[4px] flex flex-wrap items-center gap-[8px]">
        <UiButton v-if="state.canRetry" :disabled="busy" @click="retry">
          {{ t('storageRecovery.retry') }}
        </UiButton>
        <UiButton :disabled="busy" @click="chooseNew">{{ t('storageRecovery.choose') }}</UiButton>
        <UiButton v-if="state.canUseDefault" :disabled="busy" @click="defaultConfirmVisible = true">
          {{ t('storageRecovery.useDefault') }}
        </UiButton>
        <span class="flex-1"></span>
        <UiButton :disabled="busy" @click="restartNow">{{ t('storageRecovery.restart') }}</UiButton>
      </div>
    </div>

    <!-- 改用默认数据环境确认（明确告知会打开另一套环境；不删原数据） -->
    <UiModal
      :open="defaultConfirmVisible"
      :title="t('storageRecovery.useDefaultTitle')"
      size="md"
      @close="defaultConfirmVisible = false"
    >
      <div class="flex flex-col gap-sm">
        <p class="text-body text-secondary dark:text-secondary-dark">
          {{ t('storageRecovery.useDefaultBody', { root: state?.configuredRoot ?? '' }) }}
        </p>
        <div class="mt-[4px] flex items-center justify-end gap-[8px]">
          <UiButton @click="defaultConfirmVisible = false">{{
            t('settings.storageCancel')
          }}</UiButton>
          <UiButton :disabled="busy" @click="act('use-default')">
            {{ t('storageRecovery.useDefaultConfirm') }}
          </UiButton>
        </div>
      </div>
    </UiModal>
  </div>
</template>
