<script setup lang="ts">
/**
 * HTTP/WS 调试 · 主容器（接口列表管理中枢）
 * 方法下拉含 WS 同级（HttpPanel 内动态渲染），接口列表 HTTP/WS 共用（SQLite 持久化）。
 */
import { useDataRefresh } from '@/core/dataTransfer/useDataRefresh'
import { onMounted, ref } from 'vue'
import type { ApiRecord } from './contracts'
import { ipc } from './ipc'
import HttpPanel from './HttpPanel.vue'
import ApiSidebar from './ApiSidebar.vue'
import type { ApiDraft } from './useHttp'
import { useHttpWsToolLifecycle } from './toolLifecycle'
import { useUiStore } from '@/stores/ui'
import { UiButton, UiInput, UiModal } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'

const ui = useUiStore()
// 工具资源生命周期：关闭页签/退出时关闭全部 WebSocket 会话（T10-4）
useHttpWsToolLifecycle()

const panel = ref<InstanceType<typeof HttpPanel> | null>(null)

/* 接口列表 */
const apis = ref<ApiRecord[]>([])
const activeApiId = ref<number | null>(null)
const saveNameOpen = ref(false)
const saveName = ref('')
const renameMode = ref(false)

async function loadApis() {
  try {
    apis.value = await ipc.apiList()
  } catch (error) {
    ui.toast(`读取接口列表失败：${error instanceof Error ? error.message : String(error)}`)
  }
}

/** 待删除的接口（两段式确认，与全站删除交互一致） */
const deleteTarget = ref<ApiRecord | null>(null)

function requestDeleteApi(id: number) {
  deleteTarget.value = apis.value.find((a) => a.id === id) ?? null
}

async function confirmDeleteApi() {
  const target = deleteTarget.value
  deleteTarget.value = null
  if (!target) return
  try {
    await ipc.apiDelete(target.id)
    if (activeApiId.value === target.id) activeApiId.value = null
    await loadApis()
    ui.toast(`已删除接口「${target.name || target.url}」`)
  } catch (error) {
    ui.toast(`删除接口失败：${error}`)
  }
}

/** 当前面板草稿 */
function currentDraft(): ApiDraft | null {
  return panel.value?.getDraft() ?? null
}

/** 新建接口：清空表单 */
function newApi() {
  activeApiId.value = null
  saveNameOpen.value = false
  saveName.value = ''
  const empty: ApiDraft = {
    type: 'http',
    method: '',
    url: '',
    params: [],
    headers: [],
    bodyMode: 'none',
    body: '',
  }
  panel.value?.applyDraft(empty)
}

/** 点击接口：加载到表单（面板内自动切换方法/WS 视图） */
function applyApi(a: ApiRecord) {
  const draft: ApiDraft = {
    type: a.type,
    method: a.method,
    url: a.url,
    params: safeParse(a.params),
    headers: safeParse(a.headers),
    bodyMode: (a.bodyMode as 'none' | 'json' | 'text') || 'none',
    body: a.body || '',
  }
  panel.value?.applyDraft(draft)
  activeApiId.value = a.id
  saveName.value = a.name
  saveNameOpen.value = false
  renameMode.value = false
}

/** 重命名接口：打开命名对话框（预填当前名） */
function renameApi(a: ApiRecord) {
  activeApiId.value = a.id
  saveName.value = a.name
  renameMode.value = true
  saveNameOpen.value = true
}

function safeParse(json: string): never[] | { id: string; key: string; value: string }[] {
  try {
    const v = JSON.parse(json || '[]')
    return Array.isArray(v) ? v : []
  } catch {
    return []
  }
}

/** 保存当前请求为接口（保存按钮在面板请求行，命名走对话框） */
async function saveApi() {
  const draft = currentDraft()
  if (!draft) {
    ui.toast('当前面板暂无可保存的内容')
    return
  }
  const name =
    saveName.value.trim() ||
    (activeApiId.value ? apis.value.find((a) => a.id === activeApiId.value)?.name || '' : '')
  if (!name) {
    saveNameOpen.value = true
    return
  }
  try {
    const id = await ipc.apiSave({
      id: activeApiId.value ?? 0,
      kind: draft.type,
      name,
      method: draft.method,
      url: draft.url.trim(),
      params: JSON.stringify(draft.params),
      headers: JSON.stringify(draft.headers),
      bodyMode: draft.bodyMode,
      body: draft.body,
    })
    activeApiId.value = id
    saveNameOpen.value = false
    renameMode.value = false
    await loadApis()
    ui.toast(activeApiId.value ? `已更新接口「${name}」` : `已保存接口「${name}」`)
  } catch (e) {
    ui.toast('保存失败：' + (e instanceof Error ? e.message : String(e)))
  }
}

/** 保存摘要（对话框内显示） */
function draftSummary(): string {
  const d = currentDraft()
  if (!d) return ''
  return d.type === 'ws' ? `WS  ${d.url}` : `${d.method}  ${d.url}`
}

onMounted(loadApis)
useDataRefresh('http_ws.', loadApis)
</script>

<template>
  <div class="flex h-full min-h-0 w-full flex-col gap-[10px]">
    <!-- 左侧接口列表 + 右侧面板 -->
    <div class="flex min-h-0 flex-1 gap-[12px]">
      <ApiSidebar
        :apis="apis"
        :active-id="activeApiId"
        @select="applyApi"
        @rename="renameApi"
        @delete="requestDeleteApi"
        @new="newApi"
      />

      <div class="min-h-0 flex-1">
        <HttpPanel ref="panel" class="h-full" @save="saveApi" />
      </div>
    </div>

    <!-- 命名对话框（保存新接口 / 更新接口） -->
    <UiModal
      :open="saveNameOpen"
      size="sm"
      :title="renameMode ? '重命名接口' : activeApiId ? '更新接口' : '保存为接口'"
      @close="saveNameOpen = false"
    >
      <p
        class="mb-[12px] truncate font-mono text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        {{ draftSummary() }}
      </p>
      <UiInput
        v-model="saveName"
        placeholder="接口名称，如：获取用户列表"
        spellcheck="false"
        autofocus
        @keyup.enter="saveApi"
      />
      <template #footer>
        <UiButton variant="ghost" @click="saveNameOpen = false">取消</UiButton>
        <UiButton variant="primary" @click="saveApi">保存</UiButton>
      </template>
    </UiModal>

    <!-- 删除接口确认（两段式，与全站删除交互一致） -->
    <ConfirmDialog
      :open="deleteTarget !== null"
      title="删除接口"
      :message="`确定删除接口「${deleteTarget?.name || deleteTarget?.url}」？该操作不可恢复。`"
      confirm-label="删除"
      danger
      @confirm="confirmDeleteApi"
      @close="deleteTarget = null"
    />
  </div>
</template>
