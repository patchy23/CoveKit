<script setup lang="ts">
/**
 * 导出向导（sync L2）
 *
 * 四步：选数据 → 确认敏感内容 → 保存位置与密码 → 结果。
 * 为什么要分步而不是一屏：密码与「包内含凭证」是两件必须被看见的事，
 * 挤在一屏里用户会直接点「确定」；分开后每一步的代价都明确。
 *
 * 口径：
 * - 「下一步」由 useSteps 按条件禁用（空选择、未确认敏感内容、密码不足 8 位都走不过去）；
 * - 密码只在本组件内存在（`ref`），关闭即丢弃，store 不持有；
 * - 失败一律留在当前步并已由 store 提示（返回 false），不出现「点了没反应」。
 */
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { UiAlert, UiButton, UiCheckbox, UiEmptyState, UiField, UiInput, UiModal } from '@/core/ui'
import { useSteps } from '@/core/ui/useSteps'
import { defaultPackFileName, fileDialog } from '@/core/dataTransfer/fileDialog'
import { useDataTransferStore } from '@/stores/dataTransfer'
import { useUiStore } from '@/stores/ui'
import { carriesSecret, closurePreview, dependencyLabelKey, profileEntries } from './packSelection'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ (event: 'update:open', value: boolean): void }>()

const { t } = useI18n()
const store = useDataTransferStore()
const ui = useUiStore()

/** 数据包密码（只在本组件内存里；不出组件、不进 store） */
const password = ref('')
/** 保存位置（选择对话框返回；取消时保持原值） */
const savePath = ref('')
/** 本地校验提示（密码不足 8 位等） */
const localError = ref('')

const steps = useSteps(['select', 'secret', 'file', 'report'] as const, {
  canAdvance: (step) => {
    if (step === 'select') return store.canExport
    if (step === 'secret')
      return !carriesSecret(store.catalog, store.choice) || store.choice.acknowledgedSecret
    if (step === 'file')
      return password.value.trim().length >= 8 && savePath.value.trim().length > 0
    return false
  },
})

const profiles = computed(() => profileEntries(store.catalog))
const pulls = computed(() => closurePreview(store.catalog, store.choice))
const carriesSecretNow = computed(() => carriesSecret(store.catalog, store.choice))
const busy = computed(() => store.busy === 'export')
/** 报告里的总条数（各数据集相加；界面只说「多少条」，不替代分项） */
const totalRecords = computed(() =>
  Object.values(store.exportReport?.counts ?? {}).reduce((sum, value) => sum + value, 0)
)

/** 打开时刷新目录并回到首步（跨空间/换数据后勾选必须重来） */
watch(
  () => props.open,
  async (open) => {
    if (!open) return
    password.value = ''
    localError.value = ''
    steps.reset()
    await store.loadCatalog()
  }
)

function close(): void {
  emit('update:open', false)
}

/** 勾选/取消某个档案 */
function toggleProfile(id: string, next: boolean): void {
  const set = new Set(store.choice.profileIds)
  if (next) set.add(id)
  else set.delete(id)
  store.choice.profileIds = profiles.value.filter((item) => set.has(item.id)).map((item) => item.id)
}

async function choosePath(): Promise<void> {
  // 建议名带空间名与导出日期：多份包放同一目录不会互相覆盖，用户在文件管理器里也认得出
  const picked = await fileDialog.pickSavePath({
    defaultPath: defaultPackFileName(store.sourceSpaceName, new Date()),
  })
  if (picked) savePath.value = picked
}

/**
 * 在文件管理器中定位刚导出的包。
 * 导出后用户第一件事就是去目录里找这个文件，让他自己翻路径是没必要的往返；
 * 失败必须可见（提示），不静默吞掉（打开器被系统禁用等情况）。
 */
async function revealPack(): Promise<void> {
  const path = store.exportReport?.path
  if (!path) return
  try {
    await revealItemInDir(path)
  } catch (reason) {
    ui.toast(
      t('settings.dataManagement.openInFolderFailed', {
        message: reason instanceof Error ? reason.message : String(reason),
      })
    )
  }
}

async function runExport(): Promise<void> {
  localError.value = ''
  if (password.value.trim().length < 8) {
    localError.value = t('settings.dataManagement.passwordLabel')
    return
  }
  const ok = await store.exportPack(password.value, savePath.value)
  if (ok) {
    password.value = ''
    // 直接跳到结果步：此时密码已清空，`goTo` 会因首步条件不满足而被挡下（它只走「沿途都可前进」的路径）
    steps.index.value = steps.steps.indexOf('report')
  }
}
</script>

<template>
  <UiModal
    :open="props.open"
    size="xl"
    :title="t('settings.dataManagement.export')"
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

    <!-- ① 选数据 -->
    <div v-if="steps.current.value === 'select'" class="space-y-3">
      <p class="text-body-sm text-text-muted">
        {{ t('settings.dataManagement.exportSelectHint') }}
      </p>
      <UiEmptyState v-if="profiles.length === 0" :title="t('settings.dataManagement.noProfiles')" />
      <div v-else class="max-h-72 space-y-2 overflow-y-auto pr-1">
        <UiCheckbox
          v-for="item in profiles"
          :key="item.id"
          :model-value="store.choice.profileIds.includes(item.id)"
          :label="item.label"
          :description="item.note ? `${item.detail} · ${item.note}` : item.detail"
          @update:model-value="toggleProfile(item.id, $event)"
        />
      </div>
      <div class="space-y-2 border-t border-border pt-3">
        <UiCheckbox
          v-model="store.choice.includeFavorites"
          :label="t('settings.dataManagement.favorites')"
        />
        <UiCheckbox
          v-model="store.choice.includeRecentTools"
          :label="t('settings.dataManagement.recentTools')"
        />
      </div>
    </div>

    <!-- ② 确认敏感内容 -->
    <div v-else-if="steps.current.value === 'secret'" class="space-y-3">
      <UiAlert
        :tone="carriesSecretNow ? 'warning' : 'info'"
        :title="
          carriesSecretNow
            ? t('settings.dataManagement.secretTitle')
            : t('settings.dataManagement.noSecret')
        "
      >
        {{ carriesSecretNow ? t('settings.dataManagement.secretBody') : '' }}
      </UiAlert>
      <UiCheckbox
        v-model="store.choice.includeCredentials"
        :label="t('settings.dataManagement.includeCredentials')"
      />
      <UiCheckbox
        v-if="carriesSecretNow"
        v-model="store.choice.acknowledgedSecret"
        :label="t('settings.dataManagement.secretAck')"
      />
      <div v-if="pulls.length" class="rounded-md border border-border px-3 py-2">
        <p class="mb-1 text-body-sm text-text-muted">{{ t('settings.dataManagement.pulls') }}</p>
        <ul class="space-y-1 text-body-sm">
          <li v-for="item in pulls" :key="item.kind">
            {{ t(dependencyLabelKey(item.kind)) }}：{{ item.count }}
          </li>
        </ul>
      </div>
    </div>

    <!-- ③ 保存位置与密码 -->
    <div v-else-if="steps.current.value === 'file'" class="space-y-3">
      <UiField :label="t('settings.dataManagement.saveTo')">
        <div class="flex items-center gap-2">
          <UiInput :model-value="savePath" readonly class="flex-1" />
          <UiButton variant="secondary" @click="choosePath">
            {{ t('settings.dataManagement.choosePath') }}
          </UiButton>
        </div>
      </UiField>
      <UiField
        :label="t('settings.dataManagement.passwordLabel')"
        :description="t('settings.dataManagement.passwordHint')"
        :error="localError"
      >
        <UiInput v-model="password" type="password" />
      </UiField>
    </div>

    <!-- ④ 结果 -->
    <div v-else class="space-y-3">
      <UiAlert tone="success" :title="t('settings.dataManagement.reportTitle')">
        {{ t('settings.dataManagement.totalRecords', { count: totalRecords }) }}
      </UiAlert>
      <p class="break-all text-body-sm text-text-muted">{{ store.exportReport?.path }}</p>
      <UiButton v-if="store.exportReport?.path" variant="secondary" @click="revealPack">
        {{ t('settings.dataManagement.openInFolder') }}
      </UiButton>
      <p
        v-if="store.exportReport && !store.exportReport.secretIncluded"
        class="text-body-sm text-text-muted"
      >
        {{ t('settings.dataManagement.noSecret') }}
      </p>
      <p v-if="store.exportReport?.excluded.length" class="text-body-sm text-text-muted">
        {{
          t('settings.dataManagement.excluded', { items: store.exportReport.excluded.join('、') })
        }}
      </p>
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
          <UiButton
            v-if="!steps.isLast.value"
            :disabled="!steps.canGoNext.value"
            @click="steps.next()"
          >
            {{ t('settings.dataManagement.next') }}
          </UiButton>
          <UiButton
            v-if="steps.current.value === 'file'"
            :loading="busy"
            :disabled="!steps.canGoNext.value"
            @click="runExport"
          >
            {{ t('settings.dataManagement.startExport') }}
          </UiButton>
          <UiButton v-if="steps.isLast.value" @click="close">
            {{ t('settings.dataManagement.done') }}
          </UiButton>
        </div>
      </div>
    </template>
  </UiModal>
</template>
