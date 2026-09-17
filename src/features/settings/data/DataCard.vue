<script setup lang="ts">
/**
 * 数据管理分区（sync L2）
 *
 * 承担三件事：导出入口、导入入口、本机空间列表（含切换）。
 * 为什么空间列表放在这里：切换空间会改变「设置页看到的全部数据」这件事本身，
 * 它属于数据管理的语义，而不是外观或存储位置。
 *
 * 口径：
 * - 导入只创建新空间，当前空间不变；切换需要重启，因此用确认弹窗把这一点写在按钮之前；
 * - 当前空间不可「切换到自己」（按钮禁用），避免无意义的重启；
 * - 每个动作都会有可见结果（store 统一 toast），这里不吞错误。
 */
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiAlert, UiBadge, UiButton, UiEmptyState, UiListRow } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
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

const spaces = computed(() => store.spaces)
const busy = computed(() => store.busy === 'switch')

onMounted(() => {
  void store.loadSpaces()
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

    <UiAlert
      v-if="store.importReport"
      class="mt-md"
      tone="success"
      :title="t('settings.dataManagement.importReportTitle')"
    >
      {{ store.importReport.spaceName }} · {{ t('settings.dataManagement.spaceCardHint') }}
    </UiAlert>

    <ExportPackDialog v-model:open="exportOpen" />
    <ImportPackDialog v-model:open="importOpen" />

    <ConfirmDialog
      :open="switchTarget !== null"
      :title="t('settings.dataManagement.switchTitle')"
      :message="t('settings.dataManagement.switchMessage')"
      :confirm-label="t('settings.dataManagement.switch')"
      @confirm="confirmSwitch"
      @close="switchTarget = null"
    />
  </section>
</template>
