<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { UiTooltip } from '@/core/ui'
/**
 * ProfileSidebar · 档案列表栏
 * 搜索 + 列表（状态点 / 名称 / 服务器 / 代理数）+ 新建 + 右键菜单（重命名 / 复制 / 备注 /
 * 在资源管理器中显示 / 删除）+ 栏脚客户端管理入口。
 * 只负责收集用户意图并向上抛出，命令调用与状态刷新由工作台统一处理。
 */
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { UiButton, UiIcon, UiIconButton, UiListRow, UiSearchInput, UiSpinner } from '@/core/ui'
import type { FrpProfileSummary } from '../contracts'
import { statusView } from '../runtime/frpStatus'
import ProfileNameDialog from './ProfileNameDialog.vue'
import ProfileRemarkDialog from './ProfileRemarkDialog.vue'

const props = defineProps<{
  /** 档案摘要列表 */
  items: FrpProfileSummary[]
  /** 当前选中的档案文件名 */
  active: string
  /** 列表加载中 */
  loading: boolean
  /** 列表加载失败原因 */
  error: string
  /** 当前默认客户端的展示名（未设置时为空串） */
  clientLabel: string
}>()
const emit = defineEmits<{
  select: [fileName: string]
  create: [fileName: string, template: 'tcp' | 'http' | 'stcp' | 'empty']
  rename: [fileName: string, newName: string]
  duplicate: [fileName: string, newName: string]
  remark: [fileName: string, remark: string]
  remove: [fileName: string]
  /** 在系统文件管理器中定位某个档案文件 */
  reveal: [fileName: string]
  /** 打开客户端管理弹窗 */
  openClients: []
}>()

const { t } = useI18n()

/** 搜索关键字 */
const keyword = ref('')
/** 右键菜单（null = 未打开） */
const menu = ref<{ x: number; y: number; item: FrpProfileSummary } | null>(null)
/** 名称弹窗（null = 未打开） */
const nameDialog = ref<{ mode: 'create' | 'rename' | 'duplicate'; initial: string } | null>(null)
/** 备注弹窗目标（null = 未打开） */
const remarkTarget = ref<FrpProfileSummary | null>(null)
/** 待删除档案（null = 未打开确认） */
const pendingDelete = ref<FrpProfileSummary | null>(null)

/** 过滤后的列表（文件名 / 展示名 / 备注 / 服务器地址） */
const visible = computed(() => {
  const needle = keyword.value.trim().toLowerCase()
  if (needle === '') return props.items
  return props.items.filter((item) =>
    [item.fileName, item.displayName, item.remark, item.serverAddr]
      .join('\n')
      .toLowerCase()
      .includes(needle)
  )
})

/** 运行中的档案数量（栏头计数用） */
const runningCount = computed(() => props.items.filter((item) => item.state === 'running').length)

/** 右键菜单项（按当前档案状态与能力组装） */
const menuItems = computed<ContextMenuItem[]>(() => {
  const target = menu.value?.item
  if (!target) return []
  return [
    { label: t('frp.menuRename'), onClick: () => openNameDialog('rename', target) },
    { label: t('frp.menuDuplicate'), onClick: () => openNameDialog('duplicate', target) },
    { label: t('frp.menuRemark'), onClick: () => (remarkTarget.value = target) },
    { label: '', separator: true },
    // 「在资源管理器中显示」放在档案级：配置目录是固定的，从列表定位到具体那个 .toml 才有意义
    { label: t('frp.menuReveal'), onClick: () => emit('reveal', target.fileName) },
    { label: '', separator: true },
    { label: t('frp.menuDelete'), danger: true, onClick: () => (pendingDelete.value = target) },
  ]
})

/** 打开右键菜单（右键未选中项时先对齐选中态，符合列表惯例） */
function openMenu(event: MouseEvent, item: FrpProfileSummary) {
  if (item.fileName !== props.active) emit('select', item.fileName)
  menu.value = { x: event.clientX, y: event.clientY, item }
}

/** 打开名称弹窗（新建时初始名为空） */
function openNameDialog(mode: 'create' | 'rename' | 'duplicate', item?: FrpProfileSummary) {
  const initial = item ? item.fileName.replace(/\.toml$/i, '') : ''
  nameDialog.value = { mode, initial: mode === 'duplicate' ? `${initial}-copy` : initial }
}

/** 名称弹窗提交 */
function onNameSubmit(fileName: string, template: 'tcp' | 'http' | 'stcp' | 'empty') {
  const mode = nameDialog.value?.mode
  const target = menu.value?.item
  nameDialog.value = null
  if (mode === 'create') {
    emit('create', fileName, template)
    return
  }
  if (!target) return
  if (mode === 'rename') emit('rename', target.fileName, fileName)
  if (mode === 'duplicate') emit('duplicate', target.fileName, fileName)
}

/** 确认删除（软删：移入 .trash/） */
function onDeleteConfirmed() {
  const target = pendingDelete.value
  pendingDelete.value = null
  if (target) emit('remove', target.fileName)
}

/** 备注提交 */
function onRemarkSubmit(remark: string) {
  const target = remarkTarget.value
  remarkTarget.value = null
  if (target) emit('remark', target.fileName, remark)
}
</script>

<template>
  <aside
    class="flex h-full min-h-0 w-[260px] shrink-0 flex-col border-r border-border dark:border-border-dark"
  >
    <!-- 栏头：标题 + 新建 -->
    <div class="flex h-[40px] shrink-0 items-center gap-[8px] px-[10px]">
      <span class="min-w-0 flex-1 truncate text-body-sm font-semibold dark:text-primary-dark">
        {{ t('frp.profilesTitle', { count: items.length }) }}
      </span>
      <span
        v-if="runningCount > 0"
        class="shrink-0 rounded-full bg-success-soft px-[6px] text-caption text-success-strong dark:bg-success-soft-dark dark:text-success-dark"
      >
        {{ t('frp.profilesRunning', { count: runningCount }) }}
      </span>
      <UiIconButton :label="t('frp.newProfile')" size="sm" @click="openNameDialog('create')">
        <UiIcon name="plus" :size="15" />
      </UiIconButton>
    </div>

    <!-- 搜索 -->
    <div class="shrink-0 px-[10px] pb-[6px]">
      <UiSearchInput v-model="keyword" size="sm" :placeholder="t('frp.searchPlaceholder')" />
    </div>

    <!-- 列表 -->
    <UiScrollArea as-child axis="vertical">
      <div class="min-h-0 flex-1 px-[6px]">
        <div v-if="loading && items.length === 0" class="flex justify-center py-[16px]">
          <UiSpinner size="sm" />
        </div>
        <p
          v-else-if="error !== ''"
          class="px-[6px] py-[8px] text-body-sm text-danger-strong dark:text-danger-dark"
        >
          {{ error }}
        </p>
        <p
          v-else-if="visible.length === 0"
          class="px-[6px] py-[8px] text-body-sm text-text-muted dark:text-text-muted-dark"
        >
          {{ keyword.trim() === '' ? t('frp.profilesEmpty') : t('frp.profilesNoMatch') }}
        </p>
        <UiListRow
          v-for="item in visible"
          :key="item.fileName"
          :active="item.fileName === active"
          cursor="pointer"
          class="items-start gap-[8px] px-[8px] py-[6px]"
          @click="emit('select', item.fileName)"
          @contextmenu.prevent="openMenu($event, item)"
        >
          <UiTooltip :content="t(statusView(item.state).labelKey)">
            <span
              class="mt-[6px] h-[8px] w-[8px] shrink-0 rounded-full"
              :class="statusView(item.state).dotClass"
            />
          </UiTooltip>
          <span class="min-w-0 flex-1">
            <UiTooltip :content="item.fileName">
              <span class="block truncate text-body-sm dark:text-primary-dark">
                {{ item.displayName || item.fileName }}
              </span>
            </UiTooltip>
            <span
              class="mt-[1px] block truncate text-caption text-text-muted dark:text-text-muted-dark"
            >
              {{
                item.serverAddr === ''
                  ? t('frp.profileNoServer')
                  : `${item.serverAddr}:${item.serverPort}`
              }}
              · {{ t('frp.proxyCount', { count: item.proxyCount }) }}
            </span>
            <UiTooltip v-if="item.remark !== ''" :content="item.remark">
              <span
                class="mt-[1px] block truncate text-caption text-tertiary-strong dark:text-tertiary-dark"
              >
                {{ item.remark }}
              </span>
            </UiTooltip>
            <UiTooltip v-if="item.lastError" :content="item.lastError">
              <span
                class="mt-[1px] block truncate text-caption text-danger-strong dark:text-danger-dark"
              >
                {{ item.lastError }}
              </span>
            </UiTooltip>
          </span>
        </UiListRow>
      </div>
    </UiScrollArea>

    <!-- 栏脚上：客户端管理入口（默认客户端 + 管理按钮） -->
    <div
      class="flex h-[32px] shrink-0 items-center gap-[6px] border-t border-border px-[10px] dark:border-border-dark"
    >
      <UiIcon
        name="package"
        :size="13"
        class="shrink-0 text-text-muted dark:text-text-muted-dark"
      />
      <span class="min-w-0 flex-1 truncate text-caption text-text-muted dark:text-text-muted-dark">
        {{ props.clientLabel === '' ? t('frp.clientNoneAvailable') : props.clientLabel }}
      </span>
      <UiButton size="xs" variant="ghost" @click="emit('openClients')">
        {{ t('frp.clientManage') }}
      </UiButton>
    </div>

    <!-- 右键菜单 -->
    <ContextMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      size="sm"
      :items="menuItems"
      @close="menu = null"
    />

    <!-- 新建 / 重命名 / 复制 -->
    <ProfileNameDialog
      :open="nameDialog !== null"
      :mode="nameDialog?.mode ?? 'create'"
      :initial="nameDialog?.initial ?? ''"
      @submit="onNameSubmit"
      @close="nameDialog = null"
    />

    <!-- 备注 -->
    <ProfileRemarkDialog
      :open="remarkTarget !== null"
      :file-name="remarkTarget?.fileName ?? ''"
      :initial="remarkTarget?.remark ?? ''"
      @submit="onRemarkSubmit"
      @close="remarkTarget = null"
    />

    <!-- 删除确认（软删；文案写明去向） -->
    <ConfirmDialog
      :open="pendingDelete !== null"
      danger
      :title="t('frp.deleteTitle')"
      :message="t('frp.deleteMessage', { name: pendingDelete?.fileName ?? '' })"
      :confirm-label="t('frp.deleteConfirm')"
      @confirm="onDeleteConfirmed"
      @close="pendingDelete = null"
    />
  </aside>
</template>
