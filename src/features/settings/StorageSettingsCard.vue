<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/** 设置页存储位置：关闭已保存的工具后复制校验，完成后在同一窗口加载新环境。 */
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { UiButton, UiModal } from '@/core/ui'
import { formatBytes } from '@/core/format'
import { ipc } from '@/core/ipc/ipc'
import type { StorageInfo } from '@/core/ipc/contracts'
import { useUiStore } from '@/stores/ui'
import AppIcon from '@/features/ui/AppIcon.vue'
import { useStorageMaintenance } from './useStorageMaintenance'

const { t } = useI18n()
const ui = useUiStore()

/** 当前存储信息（未加载完成时为 null） */
const info = ref<StorageInfo | null>(null)
/** 读取失败原因（展示为错误行） */
const loadError = ref('')
const { busy, error: actionError, run } = useStorageMaintenance(t)
/** 执行旧计划时不重新登记目标。 */
const executePending = ref(false)
/** 待确认的目标目录 */
const pendingTarget = ref('')
/** 迁移确认弹窗 */
const confirmVisible = ref(false)

/** 分区标识 → i18n 键（四个固定分区） */
const PARTITION_KEYS: Record<string, string> = {
  data: 'settings.storagePartData',
  vault: 'settings.storagePartVault',
  logs: 'settings.storagePartLogs',
  cache: 'settings.storagePartCache',
}

/** 合计占用文案 */
const totalText = computed(() => {
  if (!info.value) return t('settings.storageLoading')
  return t('settings.storageTotal', {
    size: formatBytes(info.value.totalBytes),
    count: info.value.fileCount,
  })
})

/** 待执行计划的阶段文案 */
const pendingPhaseText = computed(() => {
  const map: Record<string, string> = {
    scheduled: 'settings.storagePendingPhaseScheduled',
    copying: 'settings.storagePendingPhaseCopying',
    verifying: 'settings.storagePendingPhaseVerifying',
  }
  return t(map[info.value?.pendingMigration?.phase ?? 'scheduled'])
})

/** 读取存储信息（进入页面与登记/取消后刷新） */
async function loadInfo() {
  loadError.value = ''
  try {
    info.value = await ipc.storageInfo()
  } catch (reason) {
    loadError.value = reason instanceof Error ? reason.message : String(reason)
  }
}

onMounted(loadInfo)

/** 选择新目录 → 打开确认弹窗 */
async function chooseTarget() {
  try {
    const selected = await dialogOpen({
      directory: true,
      multiple: false,
      defaultPath: info.value?.root,
    })
    if (typeof selected !== 'string') return
    if (selected === info.value?.root) {
      ui.toast(t('settings.storageSameDir'))
      return
    }
    executePending.value = false
    actionError.value = ''
    pendingTarget.value = selected
    confirmVisible.value = true
  } catch (reason) {
    actionError.value = reason instanceof Error ? reason.message : String(reason)
  }
}

/** 恢复默认目录 → 打开确认弹窗 */
function resetToDefault() {
  if (!info.value) return
  executePending.value = false
  actionError.value = ''
  pendingTarget.value = info.value.defaultRoot
  confirmVisible.value = true
}

/** 执行已确认的迁移，失败保留原因和计划以便重试。 */
async function migrateNow() {
  await run(() => ipc.storageMigrateNow(executePending.value ? undefined : pendingTarget.value))
  if (actionError.value) {
    await loadInfo()
    executePending.value = !!info.value?.pendingMigration
  }
}

/** 已登记计划沿用原目标，仍需要确认工具关闭。 */
function confirmPending() {
  if (!info.value?.pendingMigration) return
  executePending.value = true
  actionError.value = ''
  pendingTarget.value = info.value.pendingMigration.target
  confirmVisible.value = true
}

/** 取消待执行计划（不修改任何业务文件） */
async function cancelPending() {
  busy.value = true
  actionError.value = ''
  try {
    const cancelled = await ipc.storageCancelMigration()
    ui.toast(
      cancelled ? t('settings.storagePendingCancelled') : t('settings.storagePendingMissing')
    )
    await loadInfo()
  } catch (reason) {
    actionError.value = reason instanceof Error ? reason.message : String(reason)
    ui.toast(t('settings.storagePendingCancelFailed', { message: actionError.value }))
  } finally {
    busy.value = false
  }
}

/** 在文件管理器中打开当前存储目录 */
async function openDir() {
  if (!info.value) return
  try {
    await revealItemInDir(info.value.root)
  } catch (reason) {
    ui.toast(
      t('settings.storageOpenFailed', {
        message: reason instanceof Error ? reason.message : String(reason),
      })
    )
  }
}
</script>

<template>
  <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
    <h3 class="flex items-center gap-[8px] text-h2 font-bold dark:text-primary-dark">
      <AppIcon name="database" :size="15" class="text-tertiary-strong dark:text-tertiary-dark" />
      {{ t('settings.storage') }}
    </h3>

    <div class="mt-sm flex flex-col gap-sm">
      <!-- 当前根目录 -->
      <div class="flex items-center gap-[8px]">
        <UiTooltip :content="info?.root">
          <code
            class="min-w-0 flex-1 truncate rounded-sm bg-neutral px-[10px] py-[7px] font-mono text-body-sm dark:bg-neutral-dark dark:text-primary-dark"
            >{{ info?.root ?? '—' }}</code
          >
        </UiTooltip>
        <span
          v-if="info"
          class="shrink-0 rounded-full px-[8px] py-[2px] text-label-caps"
          :class="
            info.isDefault
              ? 'bg-neutral text-text-muted dark:bg-neutral-dark dark:text-text-muted-dark'
              : 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
          "
        >
          {{ info.isDefault ? t('settings.storageDefault') : t('settings.storageCustom') }}
        </span>
      </div>

      <!-- 四分区占用 -->
      <div
        class="flex flex-col divide-y divide-border rounded-sm border border-border dark:divide-border-dark dark:border-border-dark"
      >
        <div
          v-for="part in info?.partitions ?? []"
          :key="part.name"
          class="flex items-center gap-[8px] px-[10px] py-[6px] text-body-sm"
        >
          <span class="w-[56px] shrink-0 font-medium dark:text-primary-dark">{{
            t(PARTITION_KEYS[part.name] ?? 'settings.storagePartData')
          }}</span>
          <UiTooltip :content="part.path">
            <span class="min-w-0 flex-1 truncate text-text-muted dark:text-text-muted-dark">{{
              part.path
            }}</span>
          </UiTooltip>
          <span class="shrink-0 tabular-nums text-secondary dark:text-secondary-dark">{{
            formatBytes(part.bytes)
          }}</span>
        </div>
      </div>

      <p class="text-body-sm text-text-muted dark:text-text-muted-dark">{{ totalText }}</p>
      <p class="text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ t('settings.storageHint') }}
      </p>

      <!-- 待执行计划：可在应用内执行或取消 -->
      <div
        v-if="info?.pendingMigration"
        class="flex flex-col gap-[6px] rounded-sm border border-warning-strong/40 bg-warning-soft/40 p-[10px] dark:border-warning-dark/40 dark:bg-warning-soft-dark/30"
      >
        <p class="text-body-sm font-medium dark:text-primary-dark">
          {{ t('settings.storagePendingTitle') }}
        </p>
        <code class="block font-mono text-body-sm break-all dark:text-primary-dark">{{
          info.pendingMigration.target
        }}</code>
        <p class="text-body-sm text-text-muted dark:text-text-muted-dark">
          {{
            t('settings.storagePendingMeta', {
              phase: pendingPhaseText,
              attempts: info.pendingMigration.attempts,
            })
          }}
        </p>
        <p
          v-if="info.pendingMigration.lastError"
          class="select-text text-body-sm text-danger-strong dark:text-danger-dark"
        >
          {{ t('settings.storagePendingFailed', { message: info.pendingMigration.lastError }) }}
        </p>
        <div class="flex items-center justify-end gap-[8px]">
          <UiButton :disabled="busy" @click="confirmPending">{{
            t('settings.storagePendingExecute')
          }}</UiButton>
          <UiButton :disabled="busy" @click="cancelPending">
            {{ t('settings.storagePendingCancel') }}
          </UiButton>
        </div>
      </div>

      <!-- 操作 -->
      <div class="flex items-center gap-[8px]">
        <UiButton :disabled="busy || !info || !!info.pendingMigration" @click="chooseTarget">
          {{ t('settings.storageChange') }}
        </UiButton>
        <UiButton
          v-if="info && !info.isDefault"
          :disabled="busy || !!info.pendingMigration"
          @click="resetToDefault"
        >
          {{ t('settings.storageReset') }}
        </UiButton>
        <UiButton :disabled="!info" @click="openDir">{{ t('settings.storageOpen') }}</UiButton>
      </div>

      <p v-if="loadError" class="select-text text-body-sm text-danger-strong dark:text-danger-dark">
        {{ loadError }}
      </p>
      <p
        v-if="actionError"
        class="select-text text-body-sm text-danger-strong dark:text-danger-dark"
      >
        {{ actionError }}
      </p>
    </div>

    <!-- 迁移确认（危险/破坏性操作：给足信息 + 显式确认，禁遮罩误触由 UiModal 保证） -->
    <UiModal
      :open="confirmVisible"
      :title="t('settings.storageScheduleTitle')"
      size="md"
      @close="!busy && (confirmVisible = false)"
    >
      <div class="flex flex-col gap-sm">
        <p class="text-body text-secondary dark:text-secondary-dark">
          {{ t('settings.storageScheduleBody') }}
        </p>
        <div class="rounded-sm border border-border p-[10px] dark:border-border-dark">
          <p class="text-body-sm text-text-muted dark:text-text-muted-dark">
            {{ t('settings.storageMigrateTarget') }}
          </p>
          <code class="mt-[4px] block font-mono text-body-sm break-all dark:text-primary-dark">{{
            pendingTarget
          }}</code>
        </div>
        <p class="text-body-sm text-text-muted dark:text-text-muted-dark">
          {{ t('settings.storageScheduleNote') }}
        </p>
        <p
          v-if="actionError"
          role="alert"
          class="whitespace-pre-line text-body-sm text-danger-strong dark:text-danger-dark"
        >
          {{ actionError }}
        </p>
        <p v-if="busy" role="status" class="text-body-sm text-secondary dark:text-secondary-dark">
          {{ t('settings.storageWorking') }}
        </p>
        <div class="mt-[4px] flex items-center justify-end gap-[8px]">
          <UiButton :disabled="busy" @click="confirmVisible = false">{{
            t('settings.storageCancel')
          }}</UiButton>
          <UiButton :disabled="busy" @click="migrateNow">
            {{ t(busy ? 'settings.storageWorking' : 'settings.storageScheduleConfirm') }}
          </UiButton>
        </div>
      </div>
    </UiModal>
  </section>
</template>
