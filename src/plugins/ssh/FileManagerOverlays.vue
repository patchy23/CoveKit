<script setup lang="ts">
/**
 * FileManagerOverlays · 文件页全部浮层（右键菜单 ×2 + 输入弹窗 ×4 + 确认弹窗 ×2 + 编辑/重命名/删除弹窗组）
 * 从 FileManagerTab 拆出（300 行红线）；纯展示装配，无业务逻辑。
 */
import type { ContextMenuItem } from '@/core/ui/ContextMenu.vue'
import type { RemoteFile } from './contracts'
import type { RemoteEditing } from './useRemoteFileOps'
import type { BatchConfirm } from './useFileBatchOps'
import ContextMenu from '@/core/ui/ContextMenu.vue'
import InputDialog from '@/core/ui/InputDialog.vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import FileManagerDialogs from './FileManagerDialogs.vue'
import ChmodDialog from './ChmodDialog.vue'

interface MenuState {
  x: number
  y: number
}

defineProps<{
  /* 远程右键菜单 */
  menu: (MenuState & { target: RemoteFile | null; multi: boolean }) | null
  menuItems: ContextMenuItem[]
  /* 本地右键菜单 */
  localMenu: (MenuState & { target: RemoteFile | null; multi: boolean }) | null
  localMenuItems: ContextMenuItem[]
  /* 远程新建 */
  mkdirTarget: string | null
  newFileTarget: string | null
  /* 本地新建（'file' | 'dir' | null） */
  localCreateKind: string | null
  /* 批量确认 */
  batchConfirm: BatchConfirm | null
  batchRunning: boolean
  /* 本地重命名/删除 */
  localRenameTarget: RemoteFile | null
  localDeleteTarget: RemoteFile | null
  /* chmod */
  chmodTarget: RemoteFile | null
  /* 远程编辑/重命名/删除 */
  editing: RemoteEditing | null
  savingEdit: boolean
  renameTarget: RemoteFile | null
  deleteTarget: RemoteFile | null
}>()

const emit = defineEmits<{
  (e: 'closeMenu'): void
  (e: 'closeLocalMenu'): void
  (e: 'closeMkdir'): void
  (e: 'mkdir', name: string): void
  (e: 'closeNewFile'): void
  (e: 'newFile', name: string): void
  (e: 'closeLocalCreate'): void
  (e: 'localCreate', name: string): void
  (e: 'closeBatch'): void
  (e: 'batch'): void
  (e: 'closeLocalRename'): void
  (e: 'localRename', name: string): void
  (e: 'closeLocalDelete'): void
  (e: 'localDelete'): void
  (e: 'cancelEdit'): void
  (e: 'cancelRename'): void
  (e: 'cancelDelete'): void
  (e: 'save', content: string, force?: boolean): void
  (e: 'closeChmod'): void
  (e: 'chmod', mode: number, recursive: boolean, acknowledgeRisk: boolean): void
  (e: 'rename', name: string): void
  (e: 'delete'): void
}>()
</script>

<template>
  <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @close="emit('closeMenu')" />
  <ContextMenu
    v-if="localMenu"
    :x="localMenu.x"
    :y="localMenu.y"
    :items="localMenuItems"
    @close="emit('closeLocalMenu')"
  />
  <InputDialog
    :open="mkdirTarget !== null"
    title="新建目录"
    label="目录名"
    confirm-label="创建"
    @close="emit('closeMkdir')"
    @confirm="(n: string) => emit('mkdir', n)"
  />
  <InputDialog
    :open="newFileTarget !== null"
    title="新建文件"
    label="文件名"
    confirm-label="创建"
    @close="emit('closeNewFile')"
    @confirm="(n: string) => emit('newFile', n)"
  />
  <InputDialog
    :open="localCreateKind !== null"
    :title="localCreateKind === 'dir' ? '新建目录（本地）' : '新建文件（本地）'"
    label="名称"
    confirm-label="创建"
    @close="emit('closeLocalCreate')"
    @confirm="(n: string) => emit('localCreate', n)"
  />
  <ConfirmDialog
    :open="batchConfirm !== null"
    :title="batchConfirm?.title ?? ''"
    :message="batchConfirm?.message ?? ''"
    :confirm-label="batchConfirm?.danger ? '确认删除' : '确定'"
    :danger="batchConfirm?.danger"
    :loading="batchRunning"
    @close="emit('closeBatch')"
    @confirm="emit('batch')"
  />
  <InputDialog
    :open="localRenameTarget !== null"
    title="重命名（本地）"
    label="新名称"
    :initial-value="localRenameTarget?.name"
    confirm-label="重命名"
    @close="emit('closeLocalRename')"
    @confirm="(n: string) => emit('localRename', n)"
  />
  <ConfirmDialog
    :open="localDeleteTarget !== null"
    title="删除（本地）"
    :message="`将从本地删除 ${localDeleteTarget?.isDir ? '目录' : '文件'}「${localDeleteTarget?.name}」${localDeleteTarget?.isDir ? '（递归删除，不进回收站，不可恢复）' : '（不进回收站，不可恢复）'}。`"
    confirm-label="确认删除"
    danger
    @close="emit('closeLocalDelete')"
    @confirm="emit('localDelete')"
  />
  <ChmodDialog
    :open="chmodTarget !== null"
    :file="chmodTarget"
    @close="emit('closeChmod')"
    @confirm="(m: number, r: boolean, a: boolean) => emit('chmod', m, r, a)"
  />
  <FileManagerDialogs
    :editing="editing"
    :saving="savingEdit"
    :rename-target="renameTarget"
    :delete-target="deleteTarget"
    @cancel-edit="emit('cancelEdit')"
    @cancel-rename="emit('cancelRename')"
    @cancel-delete="emit('cancelDelete')"
    @save="(c: string, f?: boolean) => emit('save', c, f)"
    @rename="(n: string) => emit('rename', n)"
    @delete="emit('delete')"
  />
</template>
