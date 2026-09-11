<script setup lang="ts">
/**
 * 凭证管理面板（框架功能；由 SettingsModal「凭证管理 → 管理凭证」以 xl 弹窗承载）
 * 类型筛选 chip + 搜索 + 列表（名称/类型/掩码摘要/备注/更新时间）；
 * 行内无操作按钮，编辑/复制值/删除走右键菜单（删除弹窗确认）；
 * 导出/导入 .pbvault（密码 + 合并/覆盖）。明文只在 reveal 后短暂存在于前端内存。
 */
import { computed, onMounted, ref } from 'vue'
import { open as dialogOpen, save as dialogSave } from '@tauri-apps/plugin-dialog'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { useCopy } from '@/core/ui/useClipboard'
import { useUiStore } from '@/stores/ui'
import { ipc } from '@/core/ipc/ipc'
import type { Credential, CredentialKind, CredentialSummary } from '@/core/ipc/contracts'
import {
  CREDENTIAL_KINDS,
  KIND_LABEL,
  filterCredentials,
  primarySecret,
} from '@/core/vault/useVault'
import CredentialForm from '@/core/vault/CredentialForm.vue'
import { clientCredentialReferenceCount } from '@/core/vault/references'
import VaultTransferDialog from './VaultTransferDialog.vue'
import VaultToolbar from './VaultToolbar.vue'
import VaultCredentialTable from './VaultCredentialTable.vue'

const ui = useUiStore()
const { copyText } = useCopy()

// ── 列表状态 ──
const list = ref<CredentialSummary[]>([])
const loading = ref(false)
const kindFilter = ref<CredentialKind | 'all'>('all')
const query = ref('')

const filtered = computed(() => filterCredentials(list.value, kindFilter.value, query.value))

/** 类型 chip（含全部；带条数） */
const kindChips = computed(() => [
  { value: 'all' as const, label: `全部 ${list.value.length}` },
  ...CREDENTIAL_KINDS.map((k) => ({
    value: k,
    label: `${KIND_LABEL[k]} ${list.value.filter((c) => c.kind === k).length}`,
  })),
])

async function reload() {
  loading.value = true
  try {
    list.value = await ipc.vaultList()
  } catch (e) {
    ui.toast(e instanceof Error ? e.message : String(e))
  } finally {
    loading.value = false
  }
}

onMounted(reload)

// ── 右键菜单 ──
const menu = ref<{ x: number; y: number; item: CredentialSummary } | null>(null)

function openMenu(event: MouseEvent, item: CredentialSummary) {
  event.preventDefault()
  menu.value = {
    x: Math.min(event.clientX, window.innerWidth - 150),
    y: Math.min(event.clientY, window.innerHeight - 150),
    item,
  }
}

const menuItems = computed<ContextMenuItem[]>(() => {
  const item = menu.value?.item
  if (!item) return []
  return [
    { label: '编辑', onClick: () => void startEdit(item.id) },
    { label: '复制值', onClick: () => void copyValue(item.id) },
    { label: '', separator: true },
    { label: '删除', danger: true, onClick: () => void startDelete(item) },
  ]
})

// ── 编辑（reveal 明文后打开表单） ──
const formOpen = ref(false)
const editing = ref<Credential | null>(null)

async function startEdit(id: string) {
  try {
    editing.value = await ipc.vaultReveal(id)
    formOpen.value = true
  } catch (e) {
    ui.toast(e instanceof Error ? e.message : String(e))
  }
}

function startCreate() {
  editing.value = null
  formOpen.value = true
}

function onSaved() {
  void reload()
}

/** 复制主秘密（password 密码 / ssh-key 私钥 / token / accessKeySecret / custom 首个秘密条目） */
async function copyValue(id: string) {
  try {
    const credential = await ipc.vaultReveal(id)
    await copyText(primarySecret(credential))
  } catch (e) {
    ui.toast(e instanceof Error ? e.message : String(e))
  }
}

// ── 删除（确认弹窗；被引用时提示引用数） ──
const deleteTarget = ref<CredentialSummary | null>(null)
const deleteError = ref('')
const deleteReferenceCount = ref(0)

async function startDelete(item: CredentialSummary) {
  deleteError.value = ''
  const clientCount = clientCredentialReferenceCount(item.id)
  let backendCount = 0
  try {
    backendCount = await ipc.vaultReferenceCount(item.id)
  } catch {
    // 浏览器预览或后端旧版本不可用时，至少保留前端 SSH 引用警告。
  }
  deleteReferenceCount.value = clientCount + backendCount
  deleteTarget.value = item
}

function closeDelete() {
  deleteTarget.value = null
  deleteError.value = ''
  deleteReferenceCount.value = 0
}

async function confirmDelete() {
  const target = deleteTarget.value
  if (!target) return
  try {
    const result = await ipc.vaultDelete(target.id)
    if (!result.ok) {
      deleteError.value = result.error ?? '删除失败'
      return
    }
    if (deleteReferenceCount.value > 0) {
      ui.toast(`已删除，原有 ${deleteReferenceCount.value} 处引用需手动改绑`)
    } else {
      ui.toast('凭证已删除')
    }
    closeDelete()
    void reload()
  } catch (e) {
    deleteError.value = e instanceof Error ? e.message : String(e)
  }
}

// ── 导出 / 导入（.pbvault 备份，密码加密） ──
const transfer = ref<{ mode: 'export' | 'import'; path: string } | null>(null)
const transferPassword = ref('')
const importOverwrite = ref('merge')

async function startExport() {
  const path = await dialogSave({
    title: '导出凭证备份',
    defaultPath: 'patchybox-vault.pbvault',
    filters: [{ name: 'CoveKit 凭证备份', extensions: ['pbvault'] }],
  })
  if (!path) return
  transferPassword.value = ''
  transfer.value = { mode: 'export', path }
}

async function startImport() {
  const path = await dialogOpen({
    title: '导入凭证备份',
    multiple: false,
    filters: [{ name: 'CoveKit 凭证备份', extensions: ['pbvault'] }],
  })
  if (!path || typeof path !== 'string') return
  transferPassword.value = ''
  importOverwrite.value = 'merge'
  transfer.value = { mode: 'import', path }
}

const transferValid = computed(() => transferPassword.value.length >= 4)

async function confirmTransfer() {
  const t = transfer.value
  if (!t || !transferValid.value) return
  try {
    if (t.mode === 'export') {
      await ipc.vaultExport(t.path, transferPassword.value)
      ui.toast('已导出加密备份')
    } else {
      const result = await ipc.vaultImport(
        t.path,
        transferPassword.value,
        importOverwrite.value === 'overwrite'
      )
      ui.toast(
        `导入完成：新增 ${result.imported} 条${result.skipped ? `，跳过 ${result.skipped} 条` : ''}`
      )
      void reload()
    }
    transfer.value = null
  } catch (e) {
    ui.toast(e instanceof Error ? e.message : String(e))
  }
}
</script>

<template>
  <div class="flex h-[56vh] min-h-[320px] flex-col">
    <VaultToolbar
      :query="query"
      :kind-filter="kindFilter"
      :chips="kindChips"
      @create="startCreate"
      @export="startExport"
      @import="startImport"
      @update:query="query = $event"
      @update:kind-filter="kindFilter = $event"
    />

    <VaultCredentialTable
      :items="filtered"
      :loading="loading"
      :query="query"
      :kind-filter="kindFilter"
      @context="openMenu"
    />

    <!-- 右键菜单 -->
    <ContextMenu
      v-if="menu"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      size="sm"
      @close="menu = null"
    />

    <!-- 编辑/新建表单 -->
    <CredentialForm
      :open="formOpen"
      :credential="editing"
      @close="formOpen = false"
      @saved="onSaved"
    />

    <!-- 删除确认 -->
    <ConfirmDialog
      :open="deleteTarget !== null"
      title="删除凭证"
      :message="
        deleteReferenceCount > 0
          ? `「${deleteTarget?.name ?? ''}」仍被 ${deleteReferenceCount} 处工具配置引用。确定强制删除吗？相关连接会失效，需重新选择或改用手工凭据。`
          : `确定删除「${deleteTarget?.name ?? ''}」吗？此操作无法撤销。`
      "
      :error="deleteError"
      confirm-label="删除"
      danger
      @close="closeDelete"
      @confirm="confirmDelete"
    />

    <VaultTransferDialog
      :transfer="transfer"
      :password="transferPassword"
      :import-mode="importOverwrite"
      :valid="transferValid"
      @close="transfer = null"
      @confirm="confirmTransfer"
      @update:password="transferPassword = $event"
      @update:import-mode="importOverwrite = $event"
    />
  </div>
</template>
