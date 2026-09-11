<script setup lang="ts">
/**
 * FrpWorkbench · FRP 客户端工具工作台
 * 组合三块状态：档案列表（useFrpProfiles）、运行状态与日志（useFrpRuntime）、frpc 二进制（useFrpBinary）。
 * 无可用 frpc 时右侧显示引导卡；切换档案前若有未保存改动会拦截确认。
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiEmptyState, UiSpinner } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import BinarySetupCard from './binary/BinarySetupCard.vue'
import { useFrpBinary } from './binary/useFrpBinary'
import type { FrpTemplateId } from './contracts'
import ProfileDetail from './profile/ProfileDetail.vue'
import ProfileSidebar from './profile/ProfileSidebar.vue'
import { useFrpProfiles } from './profile/useFrpProfiles'
import { useFrpRuntime } from './runtime/useFrpRuntime'

const { t } = useI18n()
const binary = useFrpBinary()
const profiles = useFrpProfiles()
const runtime = useFrpRuntime()

/** 当前选中的档案文件名 */
const activeFile = ref('')
/** 详情页是否有未保存改动（切换档案前拦一道） */
const dirty = ref(false)
/** 待切换的目标档案（有脏改动时暂存） */
const pendingSwitch = ref<string | null>(null)

/** 是否缺少可用的 frpc（右侧显示引导卡） */
const needSetup = computed(() => binary.info.value === null || !binary.info.value.ok)

onMounted(async () => {
  await binary.detect()
  await profiles.refresh()
  if (profiles.items.value.length > 0) activeFile.value = profiles.items.value[0].fileName
})

/** 实时运行状态并回列表项，状态点跟随运行状态（不必等下次刷新） */
watch(
  () => runtime.states.value,
  (states) => {
    profiles.items.value = profiles.items.value.map((item) => {
      const live = states[item.fileName]
      return live === undefined ? item : { ...item, state: live.state }
    })
  },
  { deep: true }
)

/** 档案名规范化（与 Rust 侧一致：自动补 .toml） */
function normalizeName(name: string): string {
  const trimmed = name.trim()
  return trimmed.toLowerCase().endsWith('.toml') ? trimmed : `${trimmed}.toml`
}

/** 选择档案：有未保存改动时先确认 */
function selectProfile(fileName: string) {
  if (fileName === activeFile.value) return
  if (dirty.value) {
    pendingSwitch.value = fileName
    return
  }
  activeFile.value = fileName
}

/** 确认丢弃未保存改动并切换 */
function confirmSwitch() {
  if (pendingSwitch.value !== null) activeFile.value = pendingSwitch.value
  pendingSwitch.value = null
  dirty.value = false
}

/** 新建档案并选中 */
async function onCreate(fileName: string, template: FrpTemplateId) {
  if (await profiles.create(fileName, template)) activeFile.value = normalizeName(fileName)
}

/** 重命名（当前选中的档案同步改名） */
async function onRename(fileName: string, newName: string) {
  if (!(await profiles.rename(fileName, newName))) return
  if (fileName === activeFile.value) activeFile.value = normalizeName(newName)
}

/** 复制一份并选中副本 */
async function onDuplicate(fileName: string, newName: string) {
  if (await profiles.duplicate(fileName, newName)) activeFile.value = normalizeName(newName)
}

/** 删除档案（当前选中的档案删掉后落到下一个） */
async function onRemove(fileName: string) {
  const wasActive = fileName === activeFile.value
  if (!(await profiles.remove(fileName))) return
  if (!wasActive) return
  dirty.value = false
  activeFile.value = profiles.items.value[0]?.fileName ?? ''
}

/** 保存备注 */
async function onRemark(fileName: string, remark: string) {
  await profiles.setRemark(fileName, remark)
}

/** frpc 引导完成后重新探测 */
async function onBinaryChanged() {
  await binary.detect()
}
</script>

<template>
  <div class="flex h-full min-h-0">
    <!-- 左：档案列表 -->
    <ProfileSidebar
      class="w-[272px] shrink-0"
      :items="profiles.items.value"
      :active="activeFile"
      :dir="profiles.dir.value || t('frp.dirUnknown')"
      :loading="profiles.loading.value"
      :error="profiles.error.value"
      @select="selectProfile"
      @create="onCreate"
      @rename="onRename"
      @duplicate="onDuplicate"
      @remark="onRemark"
      @remove="onRemove"
    />

    <!-- 右：引导卡 / 详情 -->
    <div class="flex min-w-0 flex-1 flex-col border-l border-border dark:border-border-dark">
      <div
        v-if="profiles.loading.value && profiles.items.value.length === 0"
        class="flex flex-1 items-center justify-center"
      >
        <UiSpinner />
      </div>
      <BinarySetupCard
        v-else-if="needSetup"
        :detected="binary.info.value"
        @changed="onBinaryChanged"
      />
      <ProfileDetail
        v-else-if="activeFile !== ''"
        :file-name="activeFile"
        :state="runtime.stateOf(activeFile)"
        :busy="runtime.busy.value[activeFile] === true"
        :logs="runtime.logsOf(activeFile)"
        @start="runtime.start"
        @stop="runtime.stop"
        @restart="runtime.restart"
        @clear-logs="runtime.clearLogs"
        @update:dirty="dirty = $event"
        @changed="profiles.refresh"
      />
      <UiEmptyState
        v-else
        class="flex-1"
        :title="t('frp.profilesEmpty')"
        :description="t('frp.binaryHint')"
      />
    </div>

    <!-- 未保存改动拦截 -->
    <ConfirmDialog
      :open="pendingSwitch !== null"
      :title="t('frp.unsaved')"
      :message="t('frp.unsavedSwitch', { name: pendingSwitch ?? '' })"
      :confirm-label="t('frp.unsavedDiscard')"
      @confirm="confirmSwitch"
      @close="pendingSwitch = null"
    />
  </div>
</template>
