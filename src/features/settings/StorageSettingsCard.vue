<script setup lang="ts">
/**
 * StorageSettingsCard · 设置页「存储位置」卡片（框架级）
 *
 * 展示当前存储根目录与 data / vault / logs / cache 四分区占用；
 * 「修改位置」「恢复默认」走统一迁移流程（预检 → 复制 → 校验 → 写配置），**重启生效**。
 * 迁移只复制不删除源目录；任一步失败都不写配置，现状不变。
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { relaunch } from '@tauri-apps/plugin-process'
import { UiButton, UiModal, UiProgress } from '@/core/ui'
import { formatBytes } from '@/core/format'
import { ipc } from '@/core/ipc/ipc'
import type { StorageInfo, StorageMigrateProgress } from '@/core/ipc/contracts'
import { useUiStore } from '@/stores/ui'
import AppIcon from '@/features/ui/AppIcon.vue'

const { t } = useI18n()
const ui = useUiStore()

/** 当前存储信息（未加载完成时为 null） */
const info = ref<StorageInfo | null>(null)
/** 读取失败原因（展示为错误行） */
const loadError = ref('')
/** 迁移进行中（按钮禁用 + 进度条） */
const migrating = ref(false)
/** 迁移进度（来自 storage://progress 事件） */
const progress = ref<StorageMigrateProgress | null>(null)
/** 迁移失败原因 */
const migrateError = ref('')
/** 待确认的目标目录 */
const pendingTarget = ref('')
/** 迁移确认弹窗 */
const confirmVisible = ref(false)
/** 迁移完成弹窗（提示重启） */
const doneVisible = ref(false)
/** 进度事件取消订阅句柄 */
let unlisten: UnlistenFn | null = null

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

/** 迁移阶段文案（precheck / copy / verify / done） */
const phaseText = computed(() => {
  const map: Record<string, string> = {
    precheck: 'settings.storagePhasePrecheck',
    copy: 'settings.storagePhaseCopy',
    verify: 'settings.storagePhaseVerify',
    done: 'settings.storagePhaseDone',
  }
  return t(map[progress.value?.phase ?? 'precheck'])
})

/** 迁移进度文案（阶段 + 已复制体积） */
const progressText = computed(() =>
  t('settings.storageMigrating', {
    phase: phaseText.value,
    bytes: formatBytes(progress.value?.copiedBytes ?? 0),
  })
)

/** 读取存储信息（进入页面与迁移完成后刷新） */
async function loadInfo() {
  loadError.value = ''
  try {
    info.value = await ipc.storageInfo()
  } catch (reason) {
    loadError.value = reason instanceof Error ? reason.message : String(reason)
  }
}

onMounted(async () => {
  await loadInfo()
  // 迁移进度：Rust 侧分阶段推送，用于进度条与阶段文案
  unlisten = await listen<StorageMigrateProgress>('storage://progress', (event) => {
    progress.value = event.payload
  })
})
onBeforeUnmount(() => unlisten?.())

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

/** 执行迁移（确认后；失败不改配置，仅提示） */
async function runMigrate() {
  confirmVisible.value = false
  migrating.value = true
  migrateError.value = ''
  progress.value = null
  try {
    const result = await ipc.storageMigrate(pendingTarget.value)
    if (!result.ok) throw new Error(result.error ?? t('settings.storageUnknownError'))
    ui.toast(t('settings.storageMigrateDone'))
    await loadInfo()
    doneVisible.value = true
  } catch (reason) {
    migrateError.value = reason instanceof Error ? reason.message : String(reason)
    ui.toast(t('settings.storageMigrateFailed', { message: migrateError.value }))
  } finally {
    migrating.value = false
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

/** 立即重启应用（迁移成功后生效） */
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

      <!-- 迁移进度 -->
      <div v-if="migrating" class="flex flex-col gap-[6px]">
        <UiProgress
          :value="progress?.copiedBytes ?? 0"
          :max="progress?.totalBytes || 1"
          size="sm"
          :label="progressText"
        />
      </div>

      <!-- 操作 -->
      <div class="flex items-center gap-[8px]">
        <UiButton :disabled="migrating || !info" @click="chooseTarget">
          {{ t('settings.storageChange') }}
        </UiButton>
        <UiButton v-if="info && !info.isDefault" :disabled="migrating" @click="resetToDefault">
          {{ t('settings.storageReset') }}
        </UiButton>
        <UiButton :disabled="!info" @click="openDir">{{ t('settings.storageOpen') }}</UiButton>
      </div>

      <p v-if="loadError" class="text-body-sm text-danger-strong dark:text-danger-dark">
        {{ loadError }}
      </p>
      <p v-if="migrateError" class="text-body-sm text-danger-strong dark:text-danger-dark">
        {{ t('settings.storageMigrateFailed', { message: migrateError }) }}
      </p>
    </div>

    <!-- 迁移确认（危险/破坏性操作：给足信息 + 显式确认，禁遮罩误触由 UiModal 保证） -->
    <UiModal
      :open="confirmVisible"
      :title="t('settings.storageMigrateTitle')"
      size="md"
      @close="confirmVisible = false"
    >
      <div class="flex flex-col gap-sm">
        <p class="text-body text-secondary dark:text-secondary-dark">
          {{ t('settings.storageMigrateBody', { size: formatBytes(info?.totalBytes ?? 0) }) }}
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
          {{ t('settings.storageMigrateNote') }}
        </p>
        <div class="mt-[4px] flex items-center justify-end gap-[8px]">
          <UiButton @click="confirmVisible = false">{{ t('settings.storageCancel') }}</UiButton>
          <UiButton :disabled="migrating" @click="runMigrate">
            {{ t('settings.storageMigrateConfirm') }}
          </UiButton>
        </div>
      </div>
    </UiModal>

    <!-- 迁移完成：提示重启生效 -->
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
