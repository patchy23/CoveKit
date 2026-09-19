<script setup lang="ts">
/** 接口调试：左侧接口库，右侧独立草稿页签；整个工具关闭才清理全部会话。 */
import { computed, onMounted, onUnmounted, ref, watchEffect } from 'vue'
import { useDataRefresh } from '@/core/dataTransfer/useDataRefresh'
import { useToolLifecycle } from '@/core/lifecycle'
import {
  UiButton,
  UiIcon,
  UiIconButton,
  UiInput,
  UiModal,
  UiTabs,
  UiTabsOverflow,
  UiTreeSelect,
} from '@/core/ui'
import { apiGroupOptions } from './apiTree'
import { useTabsOverflow } from '@/core/ui/useTabsOverflow'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { useUiStore } from '@/stores/ui'
import { useApiWorkspace, type ApiTab } from './useApiWorkspace'
import type { ApiKind, ApiRecord } from './contracts'
import ApiSidebar from './ApiSidebar.vue'
import HttpPanel from './HttpPanel.vue'
import NewRequestMenu from './NewRequestMenu.vue'
const ui = useUiStore()
const lifecycle = useToolLifecycle('http-ws', {
  owner: 'http-ws.workspace',
  dispose: () => workspace.dispose(),
})
const workspace = useApiWorkspace(
  (message) => ui.toast(message),
  () =>
    lifecycle.visibility.value.active &&
    !lifecycle.visibility.value.hidden &&
    !lifecycle.visibility.value.covered
)
const { apis, groups, tabs, activeKey, current, loading, loadError } = workspace
const sidebar = ref(true)
const tabBar = ref<HTMLElement | null>(null)
const items = computed(() =>
  tabs.map((t) => ({
    value: t.key,
    label: `${workspace.dirty(t) ? '* ' : ''}${t.draft.type === 'http' ? t.draft.method : t.draft.type.toUpperCase()} ${t.name}`,
    closable: !t.saving,
    status: t.session.state.busy
      ? ('progress' as const)
      : t.session.state.connected
        ? ('success' as const)
        : undefined,
    statusTitle: t.session.state.busy ? '请求进行中' : '已连接',
  }))
)
const { visibleItems, hiddenItems } = useTabsOverflow(tabBar, items, activeKey, 130)
watchEffect(() => {
  lifecycle.dirty.value = tabs.some((t) => workspace.dirty(t))
  lifecycle.running.value =
    workspace.moving.value.size > 0 ||
    workspace.groupMoving.value ||
    tabs.some((t) => t.saving || t.session.state.busy || t.session.state.connected)
})
const naming = ref<{
  tab?: ApiTab
  record?: ApiRecord
  copy?: boolean
  closeAfter?: boolean
} | null>(null)
const name = ref(''),
  group = ref(''),
  saving = ref(false),
  dialogError = ref('')
const closing = ref<ApiTab | null>(null),
  deleting = ref<ApiRecord | null>(null)
const moveTarget = ref<{ api?: ApiRecord; path?: string } | null>(null)
const moveParent = ref(''),
  moveSaving = ref(false),
  moveError = ref('')
const groupOptions = computed(() => apiGroupOptions(groups.value))
const moveOptions = computed(() => apiGroupOptions(groups.value, moveTarget.value?.path))
function requestMove(target: { api?: ApiRecord; path?: string }) {
  moveTarget.value = target
  moveParent.value = target.api?.groupName ?? target.path?.split('/').slice(0, -1).join('/') ?? ''
  moveError.value = ''
}
async function confirmMove() {
  const target = moveTarget.value
  if (!target || moveSaving.value) return
  moveSaving.value = true
  moveError.value = ''
  try {
    if (target.api) await workspace.move(target.api.id, moveParent.value)
    else if (target.path) await workspace.moveGroup(target.path, moveParent.value)
    moveTarget.value = null
  } catch (error) {
    moveError.value = String(error)
  } finally {
    moveSaving.value = false
  }
}
async function perform(action: () => unknown) {
  try {
    await action()
  } catch (error) {
    ui.toast(String(error))
  }
}
function create(kind: ApiKind, groupName = '') {
  workspace.create(kind, groupName)
}
const groupParent = ref<string | null>(null),
  newGroupName = ref(''),
  groupSaving = ref(false),
  groupError = ref('')
function newGroup(parent: string) {
  groupParent.value = parent
  newGroupName.value = ''
  groupError.value = ''
}
async function confirmGroup() {
  if (groupParent.value === null || groupSaving.value) return
  groupSaving.value = true
  groupError.value = ''
  try {
    await workspace.createGroup(newGroupName.value, groupParent.value)
    groupParent.value = null
  } catch (error) {
    groupError.value = String(error)
  } finally {
    groupSaving.value = false
  }
}
function saveDialog(tab: ApiTab, copy = false, closeAfter = false) {
  naming.value = { tab, copy, closeAfter }
  name.value = tab.recordId && !copy ? tab.name : copy ? `${tab.name} 副本` : ''
  group.value = tab.groupName
  dialogError.value = ''
}
function requestSave(tab: ApiTab, copy = false) {
  if (tab.recordId && !copy) void perform(() => workspace.save(tab, tab.name, tab.groupName))
  else saveDialog(tab, copy)
}
function rename(record: ApiRecord) {
  naming.value = { record }
  name.value = record.name
  group.value = record.groupName || ''
  dialogError.value = ''
}
async function confirmSave() {
  const target = naming.value
  if (!target || saving.value) return
  saving.value = true
  dialogError.value = ''
  try {
    if (target.record) {
      const record = apis.value.find((api) => api.id === target.record?.id)
      if (!record) throw new Error('接口已不存在，请刷新后重试')
      await workspace.rename(record, name.value, record.groupName)
    } else if (target.tab) {
      await workspace.save(target.tab, name.value, group.value, target.copy)
      if (target.closeAfter) await workspace.close(target.tab)
    }
    naming.value = null
  } catch (error) {
    dialogError.value = String(error)
  } finally {
    saving.value = false
  }
}
function requestClose(key: string) {
  const tab = tabs.find((t) => t.key === key)
  if (!tab) return
  if (workspace.dirty(tab) || tab.session.state.connected || tab.session.state.busy)
    closing.value = tab
  else void perform(() => workspace.close(tab))
}
async function discardClose() {
  const tab = closing.value
  if (!tab) return
  await perform(async () => {
    await workspace.close(tab)
    closing.value = null
  })
}
function saveClose() {
  const tab = closing.value
  if (!tab) return
  closing.value = null
  saveDialog(tab, false, true)
}
async function confirmDelete() {
  const record = deleting.value
  if (!record) return
  await perform(async () => {
    await workspace.remove(record)
    deleting.value = null
  })
}
onMounted(workspace.load)
useDataRefresh('http_ws.', workspace.load)
onUnmounted(() => {
  void workspace.dispose().catch((error) => ui.toast(`接口连接清理失败：${String(error)}`))
})
</script>
<template>
  <div class="flex h-full min-h-0 min-w-0 text-primary dark:text-primary-dark">
    <ApiSidebar
      v-if="sidebar"
      :busy="workspace.groupMoving.value"
      :group-identities="workspace.groupIdentities.value"
      :apis="apis"
      :groups="groups"
      :active-id="current?.recordId ?? null"
      :loading="loading"
      :error="loadError"
      @tree-move="(move) => perform(() => workspace.moveTree(move))"
      @select="perform(() => workspace.open($event))"
      @rename="rename"
      @request-move="requestMove({ api: $event })"
      @request-move-group="requestMove({ path: $event })"
      @delete="deleting = $event"
      @new="create"
      @new-group="newGroup"
      @move="(id, groupName) => perform(() => workspace.move(id, groupName))"
      @move-group="(path, parent) => perform(() => workspace.moveGroup(path, parent))"
      @retry="workspace.load()"
    />
    <div class="flex min-h-0 min-w-0 flex-1 flex-col">
      <div
        ref="tabBar"
        class="flex min-h-[36px] shrink-0 items-center gap-[3px] border-b border-border px-[4px] dark:border-border-dark"
      >
        <UiIconButton
          size="xs"
          :label="sidebar ? '收起接口库' : '展开接口库'"
          @click="sidebar = !sidebar"
          ><UiIcon :name="sidebar ? 'chevrons-left' : 'chevrons-right'" :size="14" /></UiIconButton
        ><UiTabs
          v-model="activeKey"
          class="min-w-0 flex-1"
          variant="line"
          size="sm"
          :items="visibleItems"
          @close="requestClose"
        /><UiTabsOverflow
          v-if="hiddenItems.length"
          :items="hiddenItems"
          :model-value="activeKey"
          @select="activeKey = $event"
          @close="requestClose"
        /><NewRequestMenu @create="create" />
      </div>
      <HttpPanel
        v-for="tab in tabs"
        v-show="activeKey === tab.key"
        :key="tab.key"
        v-model="tab.draft"
        class="min-h-0 flex-1"
        :session="tab.session"
        :saving="tab.saving"
        :active="activeKey === tab.key"
        @save="requestSave(tab)"
        @save-as="requestSave(tab, true)"
      />
      <div
        v-if="!tabs.length"
        class="flex flex-1 flex-col items-center justify-center gap-[6px] text-body-sm text-secondary dark:text-secondary-dark"
      >
        <p>从左侧选择接口开始调试</p>
        <p class="text-caption text-text-muted dark:text-text-muted-dark">
          点击 + 新建 HTTP、SSE 或 WebSocket 接口
        </p>
      </div>
    </div>
    <UiModal
      :open="!!naming"
      size="sm"
      :title="naming?.record ? '重命名' : naming?.copy ? '另存接口' : '保存接口'"
      @close="!saving && (naming = null)"
      ><div class="space-y-[10px]">
        <UiInput
          v-model="name"
          size="sm"
          aria-label="接口名称"
          placeholder="接口名称"
          :disabled="saving"
          @keyup.enter="confirmSave"
        /><UiTreeSelect
          v-if="!naming?.record"
          v-model="group"
          :options="groupOptions"
          size="sm"
          label="所属分组"
          :disabled="saving"
        />
        <p
          v-if="dialogError"
          role="alert"
          class="text-body-sm text-tertiary-strong dark:text-tertiary-dark"
        >
          {{ dialogError }}
        </p>
      </div>
      <template #footer
        ><UiButton size="sm" variant="ghost" :disabled="saving" @click="naming = null"
          >取消</UiButton
        ><UiButton size="sm" variant="primary" :loading="saving" @click="confirmSave"
          >保存</UiButton
        ></template
      ></UiModal
    >
    <UiModal
      :open="!!moveTarget"
      size="sm"
      title="移动到…"
      @close="!moveSaving && (moveTarget = null)"
    >
      <UiTreeSelect
        v-model="moveParent"
        :options="moveOptions"
        size="sm"
        label="目标分组"
        :disabled="moveSaving"
      />
      <p
        v-if="moveError"
        role="alert"
        class="mt-[8px] text-body-sm text-danger-strong dark:text-danger-dark"
      >
        {{ moveError }}
      </p>
      <template #footer>
        <UiButton size="sm" variant="ghost" :disabled="moveSaving" @click="moveTarget = null"
          >取消</UiButton
        >
        <UiButton size="sm" variant="primary" :loading="moveSaving" @click="confirmMove"
          >移动</UiButton
        >
      </template>
    </UiModal>
    <UiModal :open="!!closing" size="sm" title="关闭接口页签" @close="closing = null"
      ><p class="text-body-sm">
        关闭「{{ closing?.name }}」？{{
          closing && workspace.dirty(closing) ? '有未保存的修改。' : ''
        }}{{
          closing?.session.state.connected || closing?.session.state.busy
            ? '当前请求或连接将停止在此页签显示，长连接会断开。'
            : ''
        }}
      </p>
      <template #footer
        ><UiButton size="sm" variant="ghost" @click="closing = null">取消</UiButton
        ><UiButton size="sm" @click="discardClose">{{
          closing && workspace.dirty(closing) ? '不保存并关闭' : '关闭'
        }}</UiButton
        ><UiButton
          v-if="closing && workspace.dirty(closing)"
          size="sm"
          variant="primary"
          @click="saveClose"
          >保存并关闭</UiButton
        ></template
      ></UiModal
    >
    <ConfirmDialog
      :open="!!deleting"
      title="删除接口"
      :message="`删除「${deleting?.name}」？已打开的内容保留为未保存草稿。`"
      confirm-label="删除"
      danger
      @confirm="confirmDelete"
      @close="deleting = null"
    />
    <UiModal
      :open="groupParent !== null"
      :title="groupParent ? '创建子分组' : '添加分组'"
      size="sm"
      @close="!groupSaving && (groupParent = null)"
    >
      <p
        v-if="groupParent"
        class="mb-[8px] break-all text-body-sm text-secondary dark:text-secondary-dark"
      >
        上级分组：{{ groupParent }}
      </p>
      <UiInput
        v-model="newGroupName"
        size="sm"
        aria-label="新分组名称"
        placeholder="分组名称"
        :disabled="groupSaving"
        @keyup.enter="confirmGroup"
      />
      <p
        v-if="groupError"
        role="alert"
        class="mt-[8px] text-body-sm text-danger-strong dark:text-danger-dark"
      >
        {{ groupError }}
      </p>
      <template #footer
        ><UiButton size="sm" variant="ghost" :disabled="groupSaving" @click="groupParent = null"
          >取消</UiButton
        ><UiButton
          size="sm"
          variant="primary"
          :loading="groupSaving"
          :disabled="!newGroupName.trim()"
          @click="confirmGroup"
          >创建</UiButton
        ></template
      >
    </UiModal>
  </div>
</template>
