<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/** 存储恢复页：显式验证既有数据目录或重试迁移，成功后应用内刷新。 */
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { UiButton, UiModal } from '@/core/ui'
import { ipc } from '@/core/ipc/ipc'
import type { StorageRecovery } from '@/core/ipc/contracts'
import { useStorageMaintenance } from './useStorageMaintenance'

const { t } = useI18n()
const { busy, error, run } = useStorageMaintenance(t)

/** 当前恢复状态（null = 正常，不显示任何界面） */
const state = ref<StorageRecovery | null>(null)
/** 改用默认环境确认弹窗（切换到默认目录中的既有空间） */
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

/** 先完成工具关闭协商，再执行恢复；只有后端确认切换完成才刷新。 */
async function act(action: 'retry' | 'use-default' | 'choose', target?: string) {
  defaultConfirmVisible.value = false
  await run(async () => {
    const result = await ipc.storageRecoveryAction(action, target)
    if (!result.ok || result.restartRequired) throw new Error(result.message)
  })
}

/** 重试当前目录或保留的迁移计划。 */
function retry() {
  void act('retry')
}

/** 失败计划可显式取消，随后允许选择原有数据目录；不删除任何迁移文件。 */
async function cancelPlan() {
  busy.value = true
  error.value = ''
  try {
    await ipc.storageCancelMigration()
    if (state.value) state.value.planId = null
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason)
  } finally {
    busy.value = false
  }
}

/** 选择原有数据所在目录；恢复操作不从当前故障目录复制数据。 */
async function chooseExisting() {
  try {
    const selected = await dialogOpen({
      directory: true,
      multiple: false,
      defaultPath: state.value?.configuredRoot,
    })
    if (typeof selected === 'string') await act('choose', selected)
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason)
  }
}
</script>

<template>
  <!--
    阻断式恢复弹窗：受控 open，close 事件不回写 state → ×/Esc 都不会真正关闭，
    用户选择重试、原有数据目录或默认目录中的既有空间。
  -->
  <UiModal :open="!!state" size="md" :title="t('storageRecovery.title')">
    <div v-if="state" class="flex flex-col gap-sm">
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

      <p v-if="busy" role="status" class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ t('settings.storageWorking') }}
      </p>
      <p
        v-if="error"
        role="alert"
        class="select-text whitespace-pre-line text-body-sm text-danger-strong dark:text-danger-dark"
      >
        {{ error }}
      </p>
    </div>
    <template v-if="state" #footer>
      <UiButton v-if="state.planId" :disabled="busy" @click="cancelPlan">
        {{ t('settings.storagePendingCancel') }}
      </UiButton>
      <UiButton v-if="state.canRetry" :disabled="busy" @click="retry">
        {{ t('storageRecovery.retry') }}
      </UiButton>
      <UiButton :disabled="busy" @click="chooseExisting">{{
        t('storageRecovery.choose')
      }}</UiButton>
      <UiButton v-if="state.canUseDefault" :disabled="busy" @click="defaultConfirmVisible = true">
        {{ t('storageRecovery.useDefault') }}
      </UiButton>
    </template>
  </UiModal>

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
</template>
