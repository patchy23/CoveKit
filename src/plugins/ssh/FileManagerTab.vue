<script setup lang="ts">
/** SSH 远程文件管理：导航、传输、危险操作与弹窗编辑。 */
import { computed, ref, watch } from "vue";
import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import type { ServerConnection, ServerProfile, RemoteFile } from "./contracts";
import { canEditRemoteFile } from "./useSsh";
import { useUiStore } from "@/stores/ui";
import ConfirmDialog from "@/core/ui/ConfirmDialog.vue";
import ContextMenu from "@/core/ui/ContextMenu.vue";
import InputDialog from "@/core/ui/InputDialog.vue";
import EditorDialog from "./EditorDialog.vue";
import FileBrowser from "./FileBrowser.vue";
import { ipc } from "./ipc";
import { useFileContextMenu } from "./useFileContextMenu";
import { useFileTransfer } from "./useFileTransfer";

const props = defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const currentPath = ref("/");
const files = ref<RemoteFile[]>([]);
const selectedFile = ref<RemoteFile | null>(null);
const transferStatus = useFileTransfer(
  () => props.connection?.sessionId,
  () => void navigate(currentPath.value)
);

const editing = ref<{ connectionId: string; path: string; content: string } | null>(null);
const savingEdit = ref(false);
const renameTarget = ref<RemoteFile | null>(null);
const deleteTarget = ref<RemoteFile | null>(null);

const parentPath = computed(() => {
  const p = currentPath.value;
  if (p === "/") return null;
  const idx = p.lastIndexOf("/");
  return idx <= 0 ? "/" : p.slice(0, idx);
});

async function navigate(path: string) {
  const connectionId = props.connection?.sessionId;
  if (!connectionId) return;
  currentPath.value = path;
  selectedFile.value = null;
  try {
    const r = await ipc.sshFileList(connectionId, path);
    if (props.connection?.sessionId !== connectionId) return;
    if (r.ok) {
      files.value = r.files;
    } else {
      ui.toast(`读取目录失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`读取目录失败：${e}`);
  }
}

function navigateUp() {
  if (parentPath.value) navigate(parentPath.value);
}

/** 双击：目录进入，符合文本规则的文件打开统一编辑弹窗。 */
async function onDoubleClick(file: RemoteFile) {
  if (file.isDir) {
    navigate(file.path);
    return;
  }
  if (!canEditRemoteFile(file)) {
    ui.toast("该文件类型或大小不支持在线编辑");
    return;
  }
  await openFile(file);
}

/** 双击与右键“编辑”共享同一打开路径。 */
async function openFile(file: RemoteFile) {
  const connectionId = props.connection?.sessionId;
  if (!connectionId) return;
  try {
    const r = await ipc.sshEditOpen(connectionId, file.path);
    if (props.connection?.sessionId !== connectionId) return;
    if (r.ok) {
      editing.value = { connectionId, path: r.path, content: r.content };
    } else {
      ui.toast(`打开文件失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`打开文件失败：${e}`);
  }
}

/** 保存：回写服务器（ssh_edit_save） */
async function onSave(content: string) {
  const target = editing.value;
  if (!target || savingEdit.value) return;
  savingEdit.value = true;
  try {
    const r = await ipc.sshEditSave(target.connectionId, target.path, content);
    if (r.ok) {
      ui.toast(`已保存 ${target.path}（${content.length} 字符）`);
      editing.value = null;
    } else {
      ui.toast(`保存失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`保存失败：${e}`);
  } finally {
    savingEdit.value = false;
  }
}

/** 上传：选择本地文件 → 传输到当前目录 */
async function upload() {
  if (!props.connection?.sessionId) return;
  let localPath: string | null;
  try {
    localPath = await dialogOpen({ multiple: false, directory: false });
  } catch (error) {
    ui.toast(`打开本地文件选择器失败：${error}`);
    return;
  }
  if (typeof localPath !== "string") return;
  const name = localPath.split(/[\\/]/).pop() ?? "file";
  const remote = currentPath.value.endsWith("/")
    ? `${currentPath.value}${name}`
    : `${currentPath.value}/${name}`;
  try {
    await ipc.sshFileUpload({
      connectionId: props.connection.sessionId,
      localPath,
      remotePath: remote,
    });
    transferStatus.value = `正在上传 ${name}`;
    ui.toast(`已开始上传：${name}`);
  } catch (e) {
    ui.toast(`上传失败：${e}`);
  }
}

/** 下载：选择保存位置 → 传输到本地 */
async function download(file: RemoteFile | null = selectedFile.value) {
  if (!file || !props.connection?.sessionId) {
    if (!file) ui.toast("请先选择文件");
    return;
  }
  let localPath: string | null;
  try {
    localPath = await dialogSave({ defaultPath: file.name });
  } catch (error) {
    ui.toast(`打开保存位置选择器失败：${error}`);
    return;
  }
  if (!localPath) return;
  try {
    await ipc.sshFileDownload({
      connectionId: props.connection.sessionId,
      remotePath: file.path,
      localPath,
    });
    transferStatus.value = `正在下载 ${file.name}`;
    ui.toast(`已开始下载：${file.name}`);
  } catch (e) {
    ui.toast(`下载失败：${e}`);
  }
}

function requestDelete(file: RemoteFile | null = selectedFile.value) {
  if (!file) {
    ui.toast("请先选择文件");
    return;
  }
  deleteTarget.value = file;
}

async function confirmDelete() {
  const file = deleteTarget.value;
  deleteTarget.value = null;
  if (!file || !props.connection?.sessionId) return;
  try {
    const r = await ipc.sshFileDelete(props.connection.sessionId, file.path, file.isDir);
    if (r.ok) {
      ui.toast(`已删除 ${file.name}`);
      selectedFile.value = null;
      navigate(currentPath.value);
    } else {
      ui.toast(`删除失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`删除失败：${e}`);
  }
}

function requestRename(file: RemoteFile | null = selectedFile.value) {
  if (!file) {
    ui.toast("请先选择文件");
    return;
  }
  renameTarget.value = file;
}

async function confirmRename(name: string) {
  const file = renameTarget.value;
  renameTarget.value = null;
  if (!file || !props.connection?.sessionId || name === file.name) return;
  const dir = file.path.slice(0, file.path.lastIndexOf("/") + 1);
  const newPath = `${dir}${name}`;
  try {
    const r = await ipc.sshFileRename(props.connection.sessionId, file.path, newPath);
    if (r.ok) {
      ui.toast(`已重命名为 ${name}`);
      selectedFile.value = null;
      navigate(currentPath.value);
    } else {
      ui.toast(`重命名失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`重命名失败：${e}`);
  }
}

const { menu, menuItems, openMenu } = useFileContextMenu({
  refresh: () => void navigate(currentPath.value),
  upload: () => void upload(),
  download: (file) => void download(file),
  edit: (file) => void openFile(file),
  rename: requestRename,
  select: (file) => (selectedFile.value = file),
});

watch(
  () => props.connection?.sessionId,
  (sessionId) => {
    files.value = [];
    selectedFile.value = null;
    editing.value = null;
    savingEdit.value = false;
    renameTarget.value = null;
    deleteTarget.value = null;
    menu.value = null;
    currentPath.value = "/";
    if (sessionId) void navigate("/");
  },
  { immediate: true }
);
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <FileBrowser
      v-model:current-path="currentPath"
      :files="files"
      :selected-path="selectedFile?.path"
      :selected-name="selectedFile?.name"
      :transfer-status="transferStatus"
      @navigate="navigate"
      @up="navigateUp"
      @upload="upload"
      @download="download()"
      @rename="requestRename()"
      @delete="requestDelete()"
      @select="selectedFile = $event"
      @open="onDoubleClick"
      @context="openMenu"
    />

    <EditorDialog
      v-if="editing"
      :path="editing.path"
      :content="editing.content"
      :saving="savingEdit"
      @save="onSave"
      @cancel="editing = null"
    />
    <ContextMenu v-if="menu" :x="menu.x" :y="menu.y" :items="menuItems" @close="menu = null" />
    <InputDialog
      :open="renameTarget !== null"
      title="重命名"
      label="新名称"
      :initial-value="renameTarget?.name"
      confirm-label="重命名"
      @close="renameTarget = null"
      @confirm="confirmRename"
    />
    <ConfirmDialog
      :open="deleteTarget !== null"
      title="删除文件"
      :message="`确定删除「${deleteTarget?.name ?? ''}」？此操作不可恢复。`"
      confirm-label="删除"
      danger
      @close="deleteTarget = null"
      @confirm="confirmDelete"
    />
  </div>
</template>
