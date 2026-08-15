<script setup lang="ts">
/**
 * 凭证管理页（框架级 workspace 页签，隐藏工具 id=vault）
 * 类型筛选 chip + 搜索 + 列表（名称/类型/掩码摘要/备注/更新时间）；
 * 行内无操作按钮，编辑/复制值/删除走右键菜单（删除弹窗确认）；
 * 导出/导入 .pbvault（密码 + 合并/覆盖）。明文只在 reveal 后短暂存在于前端内存。
 */
import { computed, onMounted, ref } from 'vue'
import {
  UiBadge,
  UiButton,
  UiInput,
  UiModal,
  UiRadioGroup,
  UiSearchInput,
  UiTable,
  UiTableCell,
} from '@/core/ui'
import ContextMenu, { type ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { useCopy } from '@/core/ui/useClipboard'
import { useUiStore } from '@/stores/ui'
import AppIcon from '@/features/ui/AppIcon.vue'
import { ipc } from '@/core/ipc/ipc'
import type { Credential, CredentialKind, CredentialSummary } from '@/core/ipc/contracts'
import {
  CREDENTIAL_KINDS,
  KIND_LABEL,
  KIND_TONE,
  filterCredentials,
  formatTimestamp,
  primarySecret,
} from '@/core/vault/useVault'
import CredentialForm from '@/core/vault/CredentialForm.vue'

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

async function startDelete(item: CredentialSummary) {
  deleteError.value = ''
  deleteTarget.value = item
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
    if (result.referencedBy > 0) {
      ui.toast(`已删除，仍有 ${result.referencedBy} 处引用需手动改绑`)
    } else {
      ui.toast('凭证已删除')
    }
    deleteTarget.value = null
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
  const { save } = await import('@tauri-apps/plugin-dialog')
  const path = await save({
    title: '导出凭证备份',
    defaultPath: 'patchybox-vault.pbvault',
    filters: [{ name: 'patchyBox 凭证备份', extensions: ['pbvault'] }],
  })
  if (!path) return
  transferPassword.value = ''
  transfer.value = { mode: 'export', path }
}

async function startImport() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const path = await open({
    title: '导入凭证备份',
    multiple: false,
    filters: [{ name: 'patchyBox 凭证备份', extensions: ['pbvault'] }],
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
  <div class="flex min-h-0 flex-1 flex-col p-[12px]">
    <!-- 头部：说明 + 搜索 + 操作 -->
    <div class="mb-[10px] flex items-center gap-[8px]">
      <div class="min-w-0 flex-1">
        <h2 class="text-h2 text-primary dark:text-primary-dark">凭证管理</h2>
        <p class="mt-[2px] text-caption text-text-muted dark:text-text-muted-dark">
          秘密加密存储在本机凭证库（keyring 主密钥），插件只引用凭证 ID，明文不出后端
        </p>
      </div>
      <UiSearchInput
        v-model="query"
        size="sm"
        placeholder="搜索名称 / 备注 / 摘要…"
        class="w-[220px]"
      />
      <UiButton size="sm" variant="secondary" @click="startImport">导入</UiButton>
      <UiButton size="sm" variant="secondary" @click="startExport">导出</UiButton>
      <UiButton size="sm" variant="primary" @click="startCreate">+ 新建凭证</UiButton>
    </div>

    <!-- 类型筛选 chip -->
    <div class="mb-[8px] flex flex-wrap gap-[6px]">
      <button
        v-for="chip in kindChips"
        :key="chip.value"
        type="button"
        class="rounded-full border px-[10px] py-[2px] text-caption transition-colors"
        :class="
          kindFilter === chip.value
            ? 'border-tertiary-strong bg-tertiary-strong/10 text-tertiary-strong'
            : 'border-border text-secondary hover:bg-border/50 dark:border-border-dark dark:text-secondary-dark'
        "
        @click="kindFilter = chip.value"
      >
        {{ chip.label }}
      </button>
    </div>

    <!-- 列表 -->
    <div class="min-h-0 flex-1 overflow-auto">
      <UiTable v-if="filtered.length" density="compact" :hoverable="true">
        <thead>
          <tr>
            <UiTableCell as="th">名称</UiTableCell>
            <UiTableCell as="th" class="w-[110px]">类型</UiTableCell>
            <UiTableCell as="th">摘要</UiTableCell>
            <UiTableCell as="th">备注</UiTableCell>
            <UiTableCell as="th" class="w-[130px]">更新时间</UiTableCell>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="item in filtered"
            :key="item.id"
            class="cursor-context-menu"
            @contextmenu="openMenu($event, item)"
          >
            <UiTableCell>{{ item.name }}</UiTableCell>
            <UiTableCell>
              <UiBadge :tone="KIND_TONE[item.kind]" size="xs">{{ KIND_LABEL[item.kind] }}</UiBadge>
            </UiTableCell>
            <UiTableCell content="technical">{{ item.masked }}</UiTableCell>
            <UiTableCell>{{ item.note || '—' }}</UiTableCell>
            <UiTableCell content="technical">{{ formatTimestamp(item.updatedAt) }}</UiTableCell>
          </tr>
        </tbody>
      </UiTable>
      <div
        v-else
        class="flex flex-col items-center gap-[8px] py-[60px] text-caption text-text-muted dark:text-text-muted-dark"
      >
        <AppIcon name="lock" :size="28" />
        <p>
          {{
            loading
              ? '加载中…'
              : query || kindFilter !== 'all'
                ? '无匹配凭证'
                : '暂无凭证，点击右上角新建'
          }}
        </p>
      </div>
    </div>

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
      :message="`确定删除「${deleteTarget?.name ?? ''}」吗？删除后引用它的工具配置将失效。`"
      confirm-label="删除"
      danger
      @close="deleteTarget = null"
      @confirm="confirmDelete"
    />

    <!-- 导出/导入密码弹窗 -->
    <UiModal
      :open="transfer !== null"
      :title="transfer?.mode === 'export' ? '导出凭证备份' : '导入凭证备份'"
      size="sm"
      @close="transfer = null"
    >
      <div class="flex flex-col gap-[10px]">
        <p class="text-body-sm text-secondary dark:text-secondary-dark">
          {{
            transfer?.mode === 'export'
              ? '备份文件使用独立密码加密（Argon2id + AES-256-GCM），导入时需输入同一密码。'
              : `从 ${transfer?.path ?? ''} 导入，需输入导出时设置的密码。`
          }}
        </p>
        <label class="field-label flex flex-col gap-[6px]">
          备份密码（至少 4 位）
          <UiInput v-model="transferPassword" type="password" placeholder="备份密码" />
        </label>
        <UiRadioGroup
          v-if="transfer?.mode === 'import'"
          v-model="importOverwrite"
          name="import-mode"
          size="sm"
          :options="[
            { value: 'merge', label: '合并', description: '保留现有凭证，同 ID 跳过' },
            { value: 'overwrite', label: '覆盖', description: '清空现有凭证后整体替换' },
          ]"
        />
        <p v-if="deleteError" class="text-caption text-danger-strong">{{ deleteError }}</p>
      </div>
      <template #footer>
        <UiButton size="sm" variant="ghost" @click="transfer = null">取消</UiButton>
        <UiButton size="sm" variant="primary" :disabled="!transferValid" @click="confirmTransfer">
          {{ transfer?.mode === 'export' ? '导出' : '导入' }}
        </UiButton>
      </template>
    </UiModal>
  </div>
</template>
