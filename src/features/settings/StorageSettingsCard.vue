<script setup lang="ts">
/**
 * StorageSettingsCard · 设置页「存储位置」卡片（框架级）
 *
 * 展示当前存储根目录与 data / vault / logs / cache 四分区占用；
 * 「修改位置」「恢复默认」只**登记迁移计划**（预检通过即返回），复制与校验在下次启动的
 * 维护阶段执行，校验通过才切换生效根，**必须重启生效**（运行期不换根、不写任何句柄）。
 * 迁移只复制不删除源目录；任一步失败都不写配置，现状不变，计划与失败原因保留可重试/取消。
 * 配置盘不可用（未插盘/只读）时由 StorageRecoveryOverlay 展示恢复入口，本卡片不再兜底降级。
 */
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { relaunch } from '@tauri-apps/plugin-process'
import { UiButton, UiModal } from '@/core/ui'
import { formatBytes } from '@/core/format'
import { ipc } from '@/core/ipc/ipc'
import type { StorageInfo } from '@/core/ipc/contracts'
import { useUiStore } from '@/stores/ui'
import AppIcon from '@/features/ui/AppIcon.vue'

const { t } = useI18n()
const ui = useUiStore()

/** 当前存储信息（未加载完成时为 null） */
const info = ref<StorageInfo | null>(null)
/** 读取失败原因（展示为错误行） */
const loadError = ref('')
/** 登记请求进行中（按钮禁用） */
const busy = ref(false)
/** 登记/取消失败原因 */
const actionError = ref('')
/** 待确认的目标目录 */
const pendingTarget = ref('')
/** 迁移确认弹窗 */
const confirmVisible = ref(false)
/** 安排完成弹窗（提示重启） */
const doneVisible = ref(false)

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
  pendingTarget.value = selected
  confirmVisible.value = true
}

/** 恢复默认目录 → 打开确认弹窗 */
function resetToDefault() {
  if (!info.value) return
  pendingTarget.value = info.value.defaultRoot
  confirmVisible.value = true
}

/** 登记迁移计划（不复制文件；重启后由维护阶段复制并校验） */
async function scheduleMigration() {
  confirmVisible.value = false
  busy.value = true
  actionError.value = ''
  try {
    const result = await ipc.storageScheduleMigration(pendingTarget.value)
    ui.toast(result.message)
    await loadInfo()
    doneVisible.value = true
  } catch (reason) {
    actionError.value = reason instanceof Error ? reason.message : String(reason)
    ui.toast(t('settings.storageScheduleFailed', { message: actionError.value }))
  } finally {
    busy.value = false
  }
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

/** 立即重启应用（迁移计划在下次启动的维护阶段执行） */
async function restartNow() {
  doneVisible.value = false
  await relaunch()
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
        <code
          class="min-w-0 flex-1 truncate rounded-sm bg-neutral px-[10px] py-[7px] font-mono text-body-sm dark:bg-neutral-dark dark:text-primary-dark"
          :title="info?.root"
          >{{ info?.root ?? '—' }}</code
        >
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
          <span
            class="min-w-0 flex-1 truncate text-text-muted dark:text-text-muted-dark"
            :title="part.path"
            >{{ part.path }}</span
          >
          <span class="shrink-0 tabular-nums text-secondary dark:text-secondary-dark">{{
            formatBytes(part.bytes)
          }}</span>
        </div>
      </div>

      <p class="text-body-sm text-text-muted dark:text-text-muted-dark">{{ totalText }}</p>
      <p class="text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ t('settings.storageHint') }}
      </p>

      <!-- 待执行计划：重启后维护阶段执行，可取消 -->
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
          class="text-body-sm text-danger-strong dark:text-danger-dark"
        >
          {{ t('settings.storagePendingFailed', { message: info.pendingMigration.lastError }) }}
        </p>
        <div class="flex items-center justify-end">
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

      <p v-if="loadError" class="text-body-sm text-danger-strong dark:text-danger-dark">
        {{ loadError }}
      </p>
      <p v-if="actionError" class="text-body-sm text-danger-strong dark:text-danger-dark">
        {{ actionError }}
      </p>
    </div>

    <!-- 迁移确认（危险/破坏性操作：给足信息 + 显式确认，禁遮罩误触由 UiModal 保证） -->
    <UiModal
      :open="confirmVisible"
      :title="t('settings.storageScheduleTitle')"
      size="md"
      @close="confirmVisible = false"
    >
      <div class="flex flex-col gap-sm">
        <p class="text-body text-secondary dark:text-secondary-dark">
          {{ t('settings.storageScheduleBody', { size: formatBytes(info?.totalBytes ?? 0) }) }}
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
        <div class="mt-[4px] flex items-center justify-end gap-[8px]">
          <UiButton @click="confirmVisible = false">{{ t('settings.storageCancel') }}</UiButton>
          <UiButton :disabled="busy" @click="scheduleMigration">
            {{ t('settings.storageScheduleConfirm') }}
          </UiButton>
        </div>
      </div>
    </UiModal>

    <!-- 安排完成：提示重启执行 -->
    <UiModal
      :open="doneVisible"
      :title="t('settings.storageDoneTitle')"
      size="md"
      @close="doneVisible = false"
    >
      <div class="flex flex-col gap-sm">
        <p class="text-body text-secondary dark:text-secondary-dark">
          {{ t('settings.storageDoneBody') }}
        </p>
        <div class="mt-[4px] flex items-center justify-end gap-[8px]">
          <UiButton @click="doneVisible = false">{{ t('settings.storageRestartLater') }}</UiButton>
          <UiButton @click="restartNow">{{ t('settings.storageRestart') }}</UiButton>
        </div>
      </div>
    </UiModal>
  </section>
</template>
