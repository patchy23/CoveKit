<script setup lang="ts">
/**
 * 凭证管理面板（框架功能；由 SettingsModal「凭证管理 → 管理凭证」以 xl 弹窗承载）
 * 类型筛选 chip + 搜索 + 列表（名称/类型/掩码摘要/备注/更新时间）；
 * 行内无操作按钮，编辑/复制值/删除走右键菜单（删除弹窗确认）；
 * 导出/导入 .pbvault（密码 + 合并/覆盖）。明文只在 reveal 后短暂存在于前端内存。
 */
import { computed, onMounted, ref } from 'vue'
import { open as dialogOpen, save as dialogSave } from '@tauri-apps/plugin-dialog'
import { UiContextMenu, type UiContextMenuItem } from '@/core/ui'
import { UiConfirmDialog } from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { useUiStore } from '@/stores/ui'
import { ipc } from '@/core/ipc/ipc'
import type { Credential, CredentialKind, CredentialSummary } from '@/core/ipc/contracts'
import {
  CREDENTIAL_KINDS,
  KIND_LABEL,
  filterCredentials,
  primarySecret,
} from '@/core/vault/useVault'
import { CredentialForm } from '@/core/vault'
import type { CredentialReferenceSummary } from '@/core/ipc/contracts'
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
    x: event.clientX,
    y: event.clientY,
    item,
  }
}

const menuItems = computed<UiContextMenuItem[]>(() => {
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

// ── 删除（确认弹窗；引用情况由各插件自报，扫描失败按「未知」显示） ──
const deleteTarget = ref<CredentialSummary | null>(null)
const deleteError = ref('')
/** 后端批量扫描得到的引用概况（查询失败时保持 null：按未知处理） */
const deleteReferences = ref<CredentialReferenceSummary | null>(null)
/** 引用查询本身失败：不允许当作「无引用」诱导删除 */
const deleteReferenceQueryFailed = ref(false)

/** 有引用，或有插件无法统计引用：需要显式强制确认 */
const deleteNeedsForce = computed(
  () =>
    deleteReferenceQueryFailed.value ||
    (deleteReferences.value?.total ?? 0) > 0 ||
    (deleteReferences.value?.unknownOwners.length ?? 0) > 0
)

/** 删除确认文案：逐条列出引用对象，扫描失败的插件明确标为未知 */
const deleteMessage = computed(() => {
  const name = deleteTarget.value?.name ?? ''
  if (deleteReferenceQueryFailed.value) {
    return `无法统计「${name}」的引用情况。删除后相关连接可能失效，确定删除吗？`
  }
  const summary = deleteReferences.value
  if (!summary) return `确定删除「${name}」吗？此操作无法撤销。`
  const lines = summary.owners
    .filter((owner) => owner.references.length > 0)
    .map((owner) => `${owner.owner}：${owner.references.map((item) => item.objectName).join('、')}`)
  if (summary.unknownOwners.length > 0) {
    lines.push(`以下插件无法统计引用：${summary.unknownOwners.join('、')}`)
  }
  if (lines.length === 0) return `确定删除「${name}」吗？此操作无法撤销。`
  return `「${name}」仍被 ${summary.total} 处配置引用。${lines.join('；')}。删除后这些连接会失效，需重新选择或改用手工凭据。确定强制删除吗？`
})

async function startDelete(item: CredentialSummary) {
  deleteError.value = ''
  deleteReferenceQueryFailed.value = false
  deleteReferences.value = null
  deleteTarget.value = item
  try {
    deleteReferences.value = await ipc.vaultCredentialReferences(item.id)
  } catch {
    // 查询失败：按未知处理，界面按「可能仍有引用」提示
    deleteReferenceQueryFailed.value = true
  }
}

function closeDelete() {
  deleteTarget.value = null
  deleteError.value = ''
  deleteReferences.value = null
  deleteReferenceQueryFailed.value = false
}

async function confirmDelete() {
  const target = deleteTarget.value
  if (!target) return
  try {
    // 带上确认时看到的引用总数：后端删除前重新核对，期间引用变化会返回错误要求重新确认
    const result = await ipc.vaultDelete(target.id, {
      force: deleteNeedsForce.value,
      expectedReferences: deleteReferences.value?.total,
    })
    if (!result.ok) {
      deleteError.value = result.error ?? '删除失败'
      return
    }
    const count = deleteReferences.value?.total ?? 0
    ui.toast(count > 0 ? `已删除，原有 ${count} 处引用需手动改绑` : '凭证已删除')
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
    defaultPath: 'covekit-vault.pbvault',
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
    <UiContextMenu
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
    <UiConfirmDialog
      :open="deleteTarget !== null"
      title="删除凭证"
      :message="deleteMessage"
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
