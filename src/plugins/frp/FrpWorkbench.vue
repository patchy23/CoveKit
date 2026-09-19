<script setup lang="ts">
/**
 * FrpWorkbench · FRP 客户端工具工作台
 * 组合三块状态：档案列表（useFrpProfiles）、运行状态与日志（useFrpRuntime）、frpc 二进制（useFrpBinary）。
 * 无可用 frpc 时右侧显示引导卡；切换档案前若有未保存改动会拦截确认。
 */
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { computed, onMounted, provide, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useUiStore } from '@/stores/ui'
import { UiEmptyState, UiSpinner } from '@/core/ui'
import { UiConfirmDialog } from '@/core/ui'
import BinarySetupCard from './binary/BinarySetupCard.vue'
import { useFrpBinary } from './binary/useFrpBinary'
import ClientManagerDialog from './client/ClientManagerDialog.vue'
import { FRP_CLIENTS_KEY } from './client/context'
import { clientTitle, hasUsableClient } from './client/frpClient'
import { useFrpClients } from './client/useFrpClients'
import type { FrpTemplateId } from './contracts'
import ProfileDetail from './profile/ProfileDetail.vue'
import ProfileSidebar from './profile/ProfileSidebar.vue'
import { useFrpProfiles } from './profile/useFrpProfiles'
import { useFrpRuntime } from './runtime/useFrpRuntime'
import { profileRuntimeView } from './runtime/frpStatus'
import { useFrpToolLifecycle } from './toolLifecycle'

const { t } = useI18n()
const ui = useUiStore()
const binary = useFrpBinary()
const profiles = useFrpProfiles()
const runtime = useFrpRuntime()
// 工具资源生命周期：关闭页签/退出时停止本工具启动的 frpc 进程（T10-4）
useFrpToolLifecycle()
// 客户端清单由工作台持有并向下注入：弹窗里改完，详情页与左栏立刻看到同一份状态
const clients = useFrpClients()

/** 当前选中的档案文件名 */
const activeFile = ref('')
/** 详情页是否有未保存改动（切换档案前拦一道） */
const dirty = ref(false)
/** 待切换的目标档案（有脏改动时暂存） */
const pendingSwitch = ref<string | null>(null)
/** 客户端管理弹窗是否可见 */
const showClients = ref(false)

provide(FRP_CLIENTS_KEY, clients)

/** 是否缺少可用的 frpc（右侧显示引导卡）。以客户端清单为准：清单里有记录但文件已丢失也算缺失 */
const needSetup = computed(() => !hasUsableClient(clients.clients.value))

/** 当前选中档案绑定的客户端 id（未绑定为 undefined，即跟随默认） */
const activeClientId = computed(
  () => profiles.items.value.find((item) => item.fileName === activeFile.value)?.clientId
)

/** 左栏栏脚展示的默认客户端名 */
const defaultClientLabel = computed(() => {
  const id = clients.defaultId.value
  if (id === undefined) return ''
  const found = clients.clients.value.find((item) => item.id === id)
  return found === undefined ? '' : clientTitle(found)
})

onMounted(async () => {
  // 先拉客户端清单：服务端会在首次调用时把既有 frpc 自动登记为默认客户端
  await clients.refresh()
  await binary.detect()
  await profiles.refresh()
  if (profiles.items.value.length > 0) activeFile.value = profiles.items.value[0].fileName
})

/** 同步状态、错误与 PID；列表刷新也不能覆盖已经收到的运行快照。 */
const sidebarItems = computed(() =>
  profiles.items.value.map((item) => profileRuntimeView(item, runtime.states.value[item.fileName]))
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

/**
 * 在系统文件管理器中定位某个档案文件。
 * 定位到具体文件而非打开整个配置目录：配置目录固定不变，那一串路径没有信息量，
 * 而「这个档案在磁盘上的哪个位置」才是从列表点进去时真正想知道的。
 * 失败必须给可见反馈（此处曾漏了事件监听，点下去毫无反应）。
 */
async function onReveal(fileName: string) {
  const dir = profiles.dir.value
  if (dir === '') {
    ui.toast(t('frp.revealFailed', { message: t('frp.dirUnknown') }))
    return
  }
  try {
    await revealItemInDir(`${dir}/${fileName}`)
  } catch (reason) {
    const message = reason instanceof Error ? reason.message : String(reason)
    ui.toast(t('frp.revealFailed', { message }))
  }
}

/** frpc 引导完成后重新探测 */
async function onBinaryChanged() {
  await binary.detect()
  await clients.refresh()
}

/** 客户端管理里改动了清单（新增 / 移除 / 换默认）：刷新档案列表以同步绑定展示 */
async function onClientsChanged() {
  await binary.detect()
  await profiles.refresh()
}
</script>

<template>
  <div class="flex h-full min-h-0">
    <!-- 左：档案列表 -->
    <ProfileSidebar
      class="w-[272px] shrink-0"
      :items="sidebarItems"
      :active="activeFile"
      :loading="profiles.loading.value"
      :error="profiles.error.value"
      :client-label="defaultClientLabel"
      @select="selectProfile"
      @create="onCreate"
      @rename="onRename"
      @duplicate="onDuplicate"
      @remark="onRemark"
      @remove="onRemove"
      @reveal="onReveal"
      @open-clients="showClients = true"
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
        :client-id="activeClientId"
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

    <!-- 客户端管理 -->
    <ClientManagerDialog
      :open="showClients"
      @changed="onClientsChanged"
      @close="showClients = false"
    />

    <!-- 未保存改动拦截 -->
    <UiConfirmDialog
      :open="pendingSwitch !== null"
      :title="t('frp.unsaved')"
      :message="t('frp.unsavedSwitch', { name: pendingSwitch ?? '' })"
      :confirm-label="t('frp.unsavedDiscard')"
      @confirm="confirmSwitch"
      @close="pendingSwitch = null"
    />
  </div>
</template>
