<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * 导入向导（sync L3）
 *
 * 四步：校验数据包 → 方式与范围 → 确认导入 → 结果。
 * 为什么先校验再选方式：包是外部文件，来源空间名、包含哪些数据集（各多少条）在解密前都不可信；
 * 用户应当先看到「这是一份什么包」，再决定导到哪里、带哪些类别进来。
 *
 * 三种方式：
 * - 新空间：写暂存目录再整体搬移，当前空间零改动；
 * - 合并到当前空间：按映射表 + 现状逐条判定，相同跳过、冲突逐条决策，原地提交不重启；
 * - 覆盖当前空间：包内容整体替换所选数据集，提交前自动快照（可还原）。
 *
 * 口径：
 * - 密码在「校验」与「确认导入」各输入一次，两处都只在本组件内存里；
 * - 合并模式的冲突默认「保留本地」，用户逐条改处置后预览就地刷新（重新规划）；
 * - 凭证永不随包：报告里的「待补录」提示用户到目标空间重填凭证。
 */
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { relaunch } from '@tauri-apps/plugin-process'
import {
  UiAlert,
  UiButton,
  UiCheckbox,
  UiField,
  UiInput,
  UiListRow,
  UiModal,
  UiRadioGroup,
} from '@/core/ui'
import { useSteps } from '@/core/ui/useSteps'
import { fileDialog } from '@/core/dataTransfer/fileDialog'
import { useDataTransferStore } from '@/stores/dataTransfer'
import type { ConflictDecision, ImportMode, ImportPlanItem } from '@/core/ipc/contracts'
import { conflictItems, groupByDecision, plannedCounts } from './importPreview'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (event: 'update:open', value: boolean): void }>()

const { t } = useI18n()
const store = useDataTransferStore()

/** 数据包密码：校验与提交各一次，两次都只在本组件内存里 */
const inspectPassword = ref('')
const commitPassword = ref('')
const packPath = ref('')
const spaceName = ref('')
/** 重复包允许导入（用户显式确认后才置真；仅新空间模式） */
const allowDuplicate = ref(false)
const localError = ref('')

const steps = useSteps(['inspect', 'plan', 'confirm', 'report'] as const, {
  canAdvance: (step) => {
    if (step === 'inspect') return store.inspected !== null
    if (step === 'plan') return store.planned !== null
    if (step === 'confirm') return commitPassword.value.trim().length >= 8
    return false
  },
})

/** 当前是否新空间模式（决定命名框与重复包确认的出现） */
const isNewSpace = computed(() => store.importMode === 'newSpace')
const groups = computed(() => groupByDecision(store.planned?.preview ?? []))
/** 冲突条目（合并模式才有；用户可逐条改处置） */
const conflicts = computed(() => conflictItems(store.planned?.preview ?? []))
/** 将写入的条数（按数据集分项求和；界面只说总数，导入完成后报告里给分项） */
const writable = computed(() =>
  plannedCounts(store.planned).reduce((sum, item) => sum + item.count, 0)
)
const duplicate = computed(() => store.inspected?.duplicate ?? null)
const busy = computed(() => ['inspect', 'plan', 'commit'].includes(store.busy))

/** 三种导入方式（UiRadioGroup 选项，展示顺序固定） */
const modeOptions = computed(() =>
  (['newSpace', 'merge', 'overwrite'] as ImportMode[]).map((value) => ({
    value,
    label: t(`settings.dataManagement.mode.${value}`),
    description: t(`settings.dataManagement.modeHint.${value}`),
  }))
)
/** 冲突处置三选一（UiRadioGroup 选项） */
const decisionOptions = computed(() =>
  (['keepLocal', 'useImported', 'keepBoth'] as ConflictDecision[]).map((value) => ({
    value,
    label: t(`settings.dataManagement.conflict.${value}`),
  }))
)
/** 导入方式的双向代理（切换即作废旧计划） */
const importModeProxy = computed({
  get: () => store.importMode,
  set: (value) => pickMode(value as ImportMode),
})

/** 结论分组标题的 i18n 键（camelCase 决策名直接拼） */
function decisionLabel(decision: ImportPlanItem['decision']): string {
  return t(`settings.dataManagement.decision.${decision}`)
}
/** 冲突条目当前选中的处置 */
function conflictOf(item: ImportPlanItem): ConflictDecision {
  const hit = store.conflicts.find(
    (entry) => entry.dataset === item.dataset && entry.sourceId === item.id
  )
  if (hit) return hit.decision
  // 没显式选过：从计划结论反推（skip=保留本地 / replace=采用导入 / keepBoth=保留两份）
  if (item.decision === 'replace') return 'useImported'
  if (item.decision === 'keepBoth') return 'keepBoth'
  return 'keepLocal'
}

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

/** 切换导入方式：旧计划作废（不同方式的判定完全不同），需重新生成预览 */
function pickMode(mode: ImportMode): void {
  if (store.importMode === mode) return
  store.importMode = mode
  store.conflicts = []
  store.planned = null
}

async function runPlan(): Promise<void> {
  localError.value = ''
  const ok = await store.planImport(spaceName.value.trim(), allowDuplicate.value)
  if (ok && !store.planned) return
  // 留在本步看预览；确认信息在确认步
}

/** 冲突处置变更：记录决策并就地重新规划（预览刷新） */
async function changeConflict(item: ImportPlanItem, decision: ConflictDecision): Promise<void> {
  store.setConflict(item.dataset, item.id, decision)
  await runPlan()
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
 * 直接切到刚导入的空间并重启（仅新空间模式）。
 * 合并/覆盖导入是原地提交、已自动刷新，没有这步。
 */
async function switchAndRestart(): Promise<void> {
  const spaceId = store.importReport?.spaceId
  if (!spaceId) return
  const switched = await store.switchSpace(spaceId)
  if (switched) await relaunch()
}

/** 勾选/取消某个数据集的导入（改动后旧计划作废） */
function toggleDataset(name: string, next: boolean): void {
  const set = new Set(store.importDatasets)
  if (next) set.add(name)
  else set.delete(name)
  store.importDatasets = [...set]
  store.planned = null
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
        <UiAlert v-if="duplicate && isNewSpace" tone="warning">
          {{
            t('settings.dataManagement.duplicate', {
              name: duplicate.spaceName,
              at: duplicate.importedAt,
            })
          }}
        </UiAlert>
      </template>
    </div>

    <!-- ② 方式与范围 -->
    <div v-else-if="steps.current.value === 'plan'" class="space-y-3">
      <UiField :label="t('settings.dataManagement.modeLabel')">
        <UiRadioGroup v-model="importModeProxy" name="import-mode" :options="modeOptions" />
      </UiField>
      <UiField v-if="isNewSpace" :label="t('settings.dataManagement.spaceNameLabel')">
        <UiInput
          v-model="spaceName"
          :placeholder="t('settings.dataManagement.spaceNamePlaceholder')"
        />
      </UiField>
      <UiCheckbox
        v-if="duplicate && isNewSpace"
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

      <!-- 预览（生成后展示；合并模式的冲突条目带处置选择） -->
      <template v-if="store.planned">
        <div
          v-for="group in groups"
          :key="group.decision"
          class="rounded-md border border-border px-3 py-2"
        >
          <p class="mb-1 text-body-sm">
            {{ decisionLabel(group.decision) }}：{{ group.items.length }}
          </p>
          <UiScrollArea as-child axis="vertical">
            <ul class="max-h-40 space-y-1 text-body-sm text-text-muted">
              <li v-for="item in group.items" :key="`${item.dataset}:${item.id}`">
                {{ item.label }}<span v-if="item.note"> · {{ item.note }}</span>
              </li>
            </ul>
          </UiScrollArea>
        </div>

        <div v-if="conflicts.length" class="rounded-md border border-border px-3 py-2">
          <p class="mb-2 text-body-sm">
            {{ t('settings.dataManagement.conflictTitle') }}：{{ conflicts.length }}
          </p>
          <div
            v-for="item in conflicts"
            :key="`conflict-${item.dataset}:${item.id}`"
            class="mb-2 border-b border-border pb-2 last:mb-0 last:border-0 last:pb-0"
          >
            <p class="mb-1 text-body-sm">
              {{ item.label }}<span class="text-text-muted"> · {{ item.note }}</span>
            </p>
            <UiRadioGroup
              :model-value="conflictOf(item)"
              :name="`conflict-${item.dataset}-${item.id}`"
              :options="
                item.dataset === 'dns.providers' || item.dataset.startsWith('settings.')
                  ? decisionOptions.filter((option) => option.value !== 'keepBoth')
                  : decisionOptions
              "
              direction="row"
              @update:model-value="changeConflict(item, $event as ConflictDecision)"
            />
          </div>
        </div>
      </template>

      <div class="flex items-center gap-2">
        <UiButton
          variant="secondary"
          :loading="store.busy === 'plan'"
          :disabled="
            (isNewSpace && !spaceName.trim()) ||
            (duplicate !== null && isNewSpace && !allowDuplicate)
          "
          @click="runPlan"
        >
          {{ t('settings.dataManagement.generatePreview') }}
        </UiButton>
        <UiButton v-if="store.planned" @click="steps.next()">
          {{ t('settings.dataManagement.next') }}
        </UiButton>
      </div>
    </div>

    <!-- ③ 确认导入 -->
    <div v-else-if="steps.current.value === 'confirm'" class="space-y-3">
      <UiAlert
        :tone="store.importMode === 'overwrite' ? 'warning' : 'info'"
        :title="store.planned?.spaceName ?? ''"
      >
        {{ t('settings.dataManagement.willWrite', { count: writable }) }}
      </UiAlert>
      <UiCheckbox
        v-model="store.importAcknowledged"
        :label="
          t(
            isNewSpace
              ? 'settings.dataManagement.importAck'
              : store.importMode === 'overwrite'
                ? 'settings.dataManagement.importAckOverwrite'
                : 'settings.dataManagement.importAckMerge'
          )
        "
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
      <p class="text-body-sm text-text-muted">
        {{
          isNewSpace
            ? t('settings.dataManagement.spaceCardHint')
            : t('settings.dataManagement.refreshedHint')
        }}
      </p>
      <UiButton
        v-if="isNewSpace && store.importReport?.spaceId"
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
