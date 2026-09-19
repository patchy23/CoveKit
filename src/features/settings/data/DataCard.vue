<script setup lang="ts">
/**
 * 数据管理分区（sync L3）
 *
 * 承担四件事：导出入口、导入入口、本机空间列表（含切换）、导入前快照还原。
 * 为什么空间列表放在这里：切换空间会改变「设置页看到的全部数据」这件事本身，
 * 它属于数据管理的语义，而不是外观或存储位置。
 *
 * 口径：
 * - 导入有三种方式：新空间（当前空间不变）/ 合并进当前空间 / 覆盖当前空间；
 *   合并与覆盖是原地提交（不重启），提交前自动快照，快照在这里提供「还原到导入前」；
 * - 切换空间仍需重启（运行时数据目录在启动时绑定），用确认弹窗把这一点写在按钮之前；
 * - 还原是破坏性动作（覆盖现状），走确认弹窗；
 * - 每个动作都会有可见结果（store 统一 toast），这里不吞错误。
 */
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiAlert, UiBadge, UiButton, UiEmptyState, UiListRow } from '@/core/ui'
import { UiConfirmDialog } from '@/core/ui'
import { useDataTransferStore } from '@/stores/dataTransfer'
import type { SpaceSummary } from '@/core/ipc/contracts'
import ExportPackDialog from './ExportPackDialog.vue'
import ImportPackDialog from './ImportPackDialog.vue'

const { t } = useI18n()
const store = useDataTransferStore()

const exportOpen = ref(false)
const importOpen = ref(false)
/** 待确认切换的目标空间（null = 没有待确认项） */
const switchTarget = ref<SpaceSummary | null>(null)
/** 待确认还原的快照目录名（null = 没有待确认项） */
const restoreTarget = ref<string | null>(null)

const spaces = computed(() => store.spaces)
const backups = computed(() => store.backups)
const busy = computed(() => store.busy === 'switch')

onMounted(() => {
  void store.loadSpaces()
  void store.loadBackups()
})

function askSwitch(space: SpaceSummary): void {
  if (space.active) return
  switchTarget.value = space
}

async function confirmSwitch(): Promise<void> {
  const target = switchTarget.value
  switchTarget.value = null
  if (target) await store.switchSpace(target.spaceId)
}

/** 确认还原：store 负责调用与提示；成功后后端会广播刷新，页面就地更新 */
async function confirmRestore(): Promise<void> {
  const dir = restoreTarget.value
  restoreTarget.value = null
  if (dir) await store.restoreBackup(dir)
}
</script>

<template>
  <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
    <h3 class="text-h2 font-bold dark:text-primary-dark">
      {{ t('settings.dataManagement.title') }}
    </h3>
    <p class="mt-1 text-body-sm text-text-muted dark:text-text-muted-dark">
      {{ t('settings.dataManagement.hint') }}
    </p>

    <div class="mt-sm flex flex-wrap items-center gap-sm">
      <UiButton @click="exportOpen = true">{{ t('settings.dataManagement.export') }}</UiButton>
      <UiButton variant="secondary" @click="importOpen = true">
        {{ t('settings.dataManagement.import') }}
      </UiButton>
    </div>

    <div class="mt-md">
      <div class="flex items-center justify-between gap-sm">
        <h4 class="text-body-sm font-semibold">{{ t('settings.dataManagement.spaces') }}</h4>
        <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
          {{ t('settings.dataManagement.spacesHint') }}
        </span>
      </div>
      <UiEmptyState v-if="spaces.length === 0" :title="t('settings.dataManagement.emptySpaces')" />
      <div v-else class="mt-2 space-y-2">
        <UiListRow v-for="space in spaces" :key="space.spaceId" :active="space.active">
          <div class="flex items-center justify-between gap-sm">
            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <span class="truncate text-body-sm">{{ space.name }}</span>
                <UiBadge v-if="space.active" tone="accent">{{
                  t('settings.dataManagement.current')
                }}</UiBadge>
              </div>
              <p class="truncate text-body-sm text-text-muted dark:text-text-muted-dark">
                <template v-if="space.sourceSpaceName">
                  {{ t('settings.dataManagement.fromSpace', { name: space.sourceSpaceName }) }}
                  <span v-if="space.importedAt"> · {{ space.importedAt }}</span>
                </template>
                <template v-else>{{ space.spaceId }}</template>
              </p>
            </div>
            <UiButton
              v-if="!space.active"
              variant="secondary"
              :disabled="busy"
              @click="askSwitch(space)"
            >
              {{ t('settings.dataManagement.switch') }}
            </UiButton>
          </div>
        </UiListRow>
      </div>
    </div>

    <!-- 导入前快照（合并/覆盖导入的后悔药；只列当前空间的） -->
    <div v-if="backups.length" class="mt-md">
      <h4 class="text-body-sm font-semibold">{{ t('settings.dataManagement.backupTitle') }}</h4>
      <div class="mt-2 space-y-2">
        <UiListRow v-for="backup in backups" :key="backup.dir">
          <div class="flex items-center justify-between gap-sm">
            <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
              {{ backup.createdAt }} ·
              {{ t('settings.dataManagement.backupFiles', { count: backup.files.length }) }}
            </span>
            <UiButton
              variant="secondary"
              :disabled="store.busy === 'restore'"
              @click="restoreTarget = backup.dir"
            >
              {{ t('settings.dataManagement.backupRestore') }}
            </UiButton>
          </div>
        </UiListRow>
      </div>
    </div>

    <UiAlert
      v-if="store.importReport"
      class="mt-md"
      tone="success"
      :title="t('settings.dataManagement.importReportTitle')"
    >
      {{ store.importReport.spaceName }}
    </UiAlert>

    <ExportPackDialog v-model:open="exportOpen" />
    <ImportPackDialog v-model:open="importOpen" />

    <UiConfirmDialog
      :open="switchTarget !== null"
      :title="t('settings.dataManagement.switchTitle')"
      :message="t('settings.dataManagement.switchMessage')"
      :confirm-label="t('settings.dataManagement.switch')"
      @confirm="confirmSwitch"
      @close="switchTarget = null"
    />
    <UiConfirmDialog
      :open="restoreTarget !== null"
      :title="t('settings.dataManagement.backupRestoreTitle')"
      :message="t('settings.dataManagement.backupRestoreMessage')"
      :confirm-label="t('settings.dataManagement.backupRestore')"
      @confirm="confirmRestore"
      @close="restoreTarget = null"
    />
  </section>
</template>
