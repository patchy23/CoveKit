<script setup lang="ts">
/**
 * 导入向导（sync L2）
 *
 * 四步：校验数据包 → 命名与范围 → 确认导入 → 结果。
 * 为什么先校验再命名：包是外部文件，来源空间名、包含哪些数据集（各多少条）在解密前都不可信；
 * 用户应当先看到「这是一份什么包」，再决定新空间叫什么、要带哪些类别进来。
 *
 * 口径：
 * - 导入只创建**新空间**，当前空间零改动（Rust 侧先写暂存目录再整体搬移）；
 * - 密码在「校验」与「确认导入」各输入一次：第二次是提交前复核文件与密码仍匹配，
 *   密码两处都只在本组件内存在；
 * - 重复包、较新 schema、未知 owner 都按「阻止该数据集 / 显式确认」处理，不静默跳过。
 */
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { relaunch } from '@tauri-apps/plugin-process'
import { UiAlert, UiButton, UiCheckbox, UiField, UiInput, UiListRow, UiModal } from '@/core/ui'
import { useSteps } from '@/core/ui/useSteps'
import { fileDialog } from '@/core/dataTransfer/fileDialog'
import { useDataTransferStore } from '@/stores/dataTransfer'
import { groupByOutcome, plannedCounts } from './importPreview'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (event: 'update:open', value: boolean): void }>()

const { t } = useI18n()
const store = useDataTransferStore()

/** 数据包密码：校验与提交各一次，两次都只在本组件内存里 */
const inspectPassword = ref('')
const commitPassword = ref('')
const packPath = ref('')
const spaceName = ref('')
/** 重复包允许导入（用户显式确认后才置真） */
const allowDuplicate = ref(false)
const localError = ref('')

const steps = useSteps(['inspect', 'plan', 'confirm', 'report'] as const, {
  canAdvance: (step) => {
    if (step === 'inspect') return store.inspected !== null
    if (step === 'plan') return spaceName.value.trim().length > 0 && store.planned !== null
    if (step === 'confirm') return commitPassword.value.trim().length >= 8
    return false
  },
})

const groups = computed(() => groupByOutcome(store.planned?.preview ?? []))
/** 将写入的条数（按数据集分项求和；界面只说总数，导入完成后报告里给分项） */
const writable = computed(() =>
  plannedCounts(store.planned).reduce((sum, item) => sum + item.count, 0)
)
const duplicate = computed(() => store.inspected?.duplicate ?? null)
const busy = computed(() => ['inspect', 'plan', 'commit'].includes(store.busy))

watch(
  () => props.open,
  (open) => {
    if (!open) return
    inspectPassword.value = ''
    commitPassword.value = ''
    packPath.value = ''
    spaceName.value = ''
    allowDuplicate.value = false
    localError.value = ''
    store.resetImport()
    steps.reset()
  }
)

function close(): void {
  emit('update:open', false)
  store.resetImport()
}

async function chooseFile(): Promise<void> {
  const picked = await fileDialog.pickOpenPath()
  if (picked) packPath.value = picked
}

async function runInspect(): Promise<void> {
  localError.value = ''
  if (!packPath.value) {
    localError.value = t('settings.dataManagement.fileLabel')
    return
  }
  if (inspectPassword.value.trim().length < 8) {
    localError.value = t('settings.dataManagement.passwordLabel')
    return
  }
  const ok = await store.inspectPack(packPath.value, inspectPassword.value)
  if (ok) {
    // 校验通过后立刻丢弃本组件里的密码：提交时会再输入一次
    inspectPassword.value = ''
    steps.next()
  }
}

async function runPlan(): Promise<void> {
  localError.value = ''
  const ok = await store.planImport(spaceName.value.trim(), allowDuplicate.value)
  if (ok) steps.next()
}

async function runCommit(): Promise<void> {
  localError.value = ''
  if (commitPassword.value.trim().length < 8) {
    localError.value = t('settings.dataManagement.passwordLabel')
    return
  }
  const ok = await store.commitImport(commitPassword.value)
  if (ok) {
    // 先跳结果步再清密码：清空后本步条件不再成立，`next()` 会被挡下
    steps.index.value = steps.steps.indexOf('report')
    commitPassword.value = ''
  }
}

/**
 * 直接切到刚导入的空间并重启。
 * 导入产物是「新空间」，用户下一步必然是想看看它——让他关掉向导回设置页、再找到空间列表手动切、
 * 再自己重启，是三次没必要的往返。切换失败时留在原步（store 已提示），不重启。
 */
async function switchAndRestart(): Promise<void> {
  const spaceId = store.importReport?.spaceId
  if (!spaceId) return
  const switched = await store.switchSpace(spaceId)
  if (switched) await relaunch()
}

/** 勾选/取消某个数据集的导入 */
function toggleDataset(name: string, next: boolean): void {
  const set = new Set(store.importDatasets)
  if (next) set.add(name)
  else set.delete(name)
  store.importDatasets = [...set]
}
</script>

<template>
  <UiModal
    :open="props.open"
    size="xl"
    :title="t('settings.dataManagement.import')"
    :close-on-backdrop="false"
    @close="close"
  >
    <div class="mb-4 flex flex-wrap items-center gap-3 text-body-sm text-text-muted">
      <span
        v-for="(step, at) in steps.steps"
        :key="step"
        :class="at === steps.index.value ? 'font-semibold text-text' : ''"
      >
        {{ at + 1 }}.
        {{ t(`settings.dataManagement.step${step.charAt(0).toUpperCase()}${step.slice(1)}`) }}
      </span>
    </div>

    <!-- ① 校验数据包 -->
    <div v-if="steps.current.value === 'inspect'" class="space-y-3">
      <UiField :label="t('settings.dataManagement.fileLabel')">
        <div class="flex items-center gap-2">
          <UiInput :model-value="packPath" readonly class="flex-1" />
          <UiButton variant="secondary" @click="chooseFile">
            {{ t('settings.dataManagement.chooseFile') }}
          </UiButton>
        </div>
      </UiField>
      <UiField
        :label="t('settings.dataManagement.passwordLabel')"
        :description="t('settings.dataManagement.passwordHint')"
        :error="localError"
      >
        <UiInput v-model="inspectPassword" type="password" />
      </UiField>
      <UiButton :loading="store.busy === 'inspect'" :disabled="!packPath" @click="runInspect">
        {{ t('settings.dataManagement.verifyTitle') }}
      </UiButton>
      <template v-if="store.inspected">
        <UiAlert tone="success" :title="t('settings.dataManagement.verifyTitle')">
          {{
            t('settings.dataManagement.fromSpace', {
              name: store.inspected.summary.sourceSpaceName,
            })
          }}
        </UiAlert>
        <UiListRow v-for="item in store.inspected.summary.datasets" :key="item.name">
          <div class="flex items-center justify-between gap-2">
            <span class="text-body-sm">{{ item.label }} · {{ item.name }}</span>
            <span class="text-body-sm text-text-muted">
              {{ item.recordCount }} ·
              {{
                item.carried
                  ? t('settings.dataManagement.carried')
                  : t('settings.dataManagement.notCarried')
              }}
            </span>
          </div>
        </UiListRow>
        <UiAlert v-if="duplicate" tone="warning">
          {{
            t('settings.dataManagement.duplicate', {
              name: duplicate.spaceName,
              at: duplicate.importedAt,
            })
          }}
        </UiAlert>
      </template>
    </div>

    <!-- ② 命名与范围 -->
    <div v-else-if="steps.current.value === 'plan'" class="space-y-3">
      <UiField :label="t('settings.dataManagement.spaceNameLabel')">
        <UiInput
          v-model="spaceName"
          :placeholder="t('settings.dataManagement.spaceNamePlaceholder')"
        />
      </UiField>
      <UiCheckbox
        v-if="duplicate"
        v-model="allowDuplicate"
        :label="t('settings.dataManagement.duplicateAllow')"
      />
      <div class="rounded-md border border-border px-3 py-2">
        <p class="mb-2 text-body-sm text-text-muted">{{ t('settings.dataManagement.stepPlan') }}</p>
        <UiCheckbox
          v-for="dataset in store.inspected?.defaults.datasets ?? []"
          :key="dataset"
          :model-value="store.importDatasets.includes(dataset)"
          :label="dataset"
          @update:model-value="toggleDataset(dataset, $event)"
        />
      </div>
      <div
        v-for="group in groups"
        :key="group.outcome"
        class="rounded-md border border-border px-3 py-2"
      >
        <p class="mb-1 text-body-sm">
          {{
            t(
              `settings.dataManagement.outcome${group.outcome === 'added' ? 'Added' : group.outcome === 'pending-reference' ? 'Pending' : 'Excluded'}`
            )
          }}：{{ group.items.length }}
        </p>
        <ul class="max-h-40 space-y-1 overflow-y-auto text-body-sm text-text-muted">
          <li v-for="item in group.items" :key="`${item.dataset}:${item.id}`">
            {{ item.label }}<span v-if="item.note"> · {{ item.note }}</span>
          </li>
        </ul>
      </div>
      <UiButton
        :loading="store.busy === 'plan'"
        :disabled="!spaceName.trim() || (duplicate !== null && !allowDuplicate)"
        @click="runPlan"
      >
        {{ t('settings.dataManagement.next') }}
      </UiButton>
    </div>

    <!-- ③ 确认导入 -->
    <div v-else-if="steps.current.value === 'confirm'" class="space-y-3">
      <UiAlert tone="info" :title="store.planned?.spaceName ?? ''">
        {{ t('settings.dataManagement.willWrite', { count: writable }) }}
      </UiAlert>
      <UiCheckbox
        v-model="store.importAcknowledged"
        :label="t('settings.dataManagement.importAck')"
      />
      <UiField
        :label="t('settings.dataManagement.passwordLabel')"
        :description="t('settings.dataManagement.passwordHint')"
        :error="localError"
      >
        <UiInput v-model="commitPassword" type="password" />
      </UiField>
      <UiButton
        :loading="store.busy === 'commit'"
        :disabled="!steps.canAdvance('confirm') || !store.importAcknowledged"
        @click="runCommit"
      >
        {{ t('settings.dataManagement.startImport') }}
      </UiButton>
    </div>

    <!-- ④ 结果 -->
    <div v-else class="space-y-3">
      <UiAlert tone="success" :title="t('settings.dataManagement.importReportTitle')">
        {{ t('settings.dataManagement.willWrite', { count: writable }) }}
      </UiAlert>
      <p class="text-body-sm">{{ store.importReport?.spaceName }}</p>
      <p class="text-body-sm text-text-muted">{{ t('settings.dataManagement.spaceCardHint') }}</p>
      <UiButton
        v-if="store.importReport?.spaceId"
        variant="secondary"
        :loading="store.busy === 'switch'"
        @click="switchAndRestart"
      >
        {{ t('settings.dataManagement.switchAndRestart') }}
      </UiButton>
      <div
        v-if="store.importReport?.pending.length"
        class="rounded-md border border-border px-3 py-2"
      >
        <p class="mb-1 text-body-sm">{{ t('settings.dataManagement.pendingTitle') }}</p>
        <ul class="space-y-1 text-body-sm text-text-muted">
          <li v-for="note in store.importReport.pending" :key="note">{{ note }}</li>
        </ul>
      </div>
    </div>

    <template #footer>
      <div class="flex items-center justify-between gap-2">
        <UiButton v-if="busy" variant="ghost" @click="store.cancelTransfer()">
          {{ t('settings.dataManagement.cancel') }}
        </UiButton>
        <span v-else />
        <div class="flex items-center gap-2">
          <UiButton
            v-if="!steps.isFirst.value && !steps.isLast.value"
            variant="secondary"
            @click="steps.back()"
          >
            {{ t('settings.dataManagement.back') }}
          </UiButton>
          <UiButton v-if="steps.isLast.value" @click="close">
            {{ t('settings.dataManagement.done') }}
          </UiButton>
        </div>
      </div>
    </template>
  </UiModal>
</template>
