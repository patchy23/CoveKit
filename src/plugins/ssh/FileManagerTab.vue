<script setup lang="ts">
/**
 * FileManagerTab · 远程文件管理子页签
 * 路径导航 + 文件列表 + 上传/下载/删除/重命名（后端 SFTP 真实数据）；
 * 双击文本文件 → 弹窗编辑 → 保存回写服务器。
 */
import { computed, ref } from "vue";
import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import type { ServerConnection, ServerProfile, RemoteFile } from "./contracts";
import { formatBytes, formatTime } from "./useSsh";
import { useUiStore } from "@/stores/ui";
import EditorDialog from "./EditorDialog.vue";
import { ipc } from "./ipc";

const props = defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const currentPath = ref("/");
const files = ref<RemoteFile[]>([]);
const selectedFile = ref<RemoteFile | null>(null);

/** 当前打开的编辑弹窗（path + 内容） */
const editing = ref<{ path: string; content: string } | null>(null);

const parentPath = computed(() => {
  const p = currentPath.value;
  if (p === "/") return null;
  const idx = p.lastIndexOf("/");
  return idx <= 0 ? "/" : p.slice(0, idx);
});

async function navigate(path: string) {
  if (!props.connection?.sessionId) return;
  currentPath.value = path;
  selectedFile.value = null;
  try {
    const r = await ipc.sshFileList(props.connection.sessionId, path);
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

/** 双击：目录进入，文件打开编辑弹窗 */
async function onDoubleClick(file: RemoteFile) {
  if (file.isDir) {
    navigate(file.path);
    return;
  }
  if (!props.connection?.sessionId) return;
  try {
    const r = await ipc.sshEditOpen(props.connection.sessionId, file.path);
    if (r.ok) {
      editing.value = { path: r.path, content: r.content };
    } else {
      ui.toast(`打开文件失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`打开文件失败：${e}`);
  }
}

/** 保存：回写服务器（ssh_edit_save） */
async function onSave(content: string) {
  const path = editing.value?.path ?? "";
  if (!props.connection?.sessionId) return;
  try {
    const r = await ipc.sshEditSave(props.connection.sessionId, path, content);
    if (r.ok) {
      ui.toast(`已保存 ${path}（${content.length} 字符）`);
      editing.value = null;
    } else {
      ui.toast(`保存失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`保存失败：${e}`);
  }
}

/** 上传：选择本地文件 → 传输到当前目录 */
async function upload() {
  if (!props.connection?.sessionId) return;
  const picked = await dialogOpen({ multiple: false, directory: false });
  if (!picked || typeof picked !== "string") return;
  const name = picked.split(/[\\/]/).pop() ?? "file";
  const remote = currentPath.value.endsWith("/")
    ? `${currentPath.value}${name}`
    : `${currentPath.value}/${name}`;
  try {
    const r = await ipc.sshFileUpload({
      connectionId: props.connection.sessionId,
      localPath: picked,
      remotePath: remote,
    });
    if (r.done) {
      ui.toast(`上传完成：${name}`);
      navigate(currentPath.value);
    }
  } catch (e) {
    ui.toast(`上传失败：${e}`);
  }
}

/** 下载：选择保存位置 → 传输到本地 */
async function download() {
  if (!selectedFile.value || !props.connection?.sessionId) {
    if (!selectedFile.value) ui.toast("请先选择文件");
    return;
  }
  const picked = await dialogSave({ defaultPath: selectedFile.value.name });
  if (!picked) return;
  try {
    const r = await ipc.sshFileDownload({
      connectionId: props.connection.sessionId,
      remotePath: selectedFile.value.path,
      localPath: picked,
    });
    if (r.done) ui.toast(`下载完成：${selectedFile.value.name}`);
  } catch (e) {
    ui.toast(`下载失败：${e}`);
  }
}

async function del() {
  if (!selectedFile.value || !props.connection?.sessionId) {
    if (!selectedFile.value) ui.toast("请先选择文件");
    return;
  }
  // 删除前确认（UI-009：与服务器删除一致，避免误删）
  if (!window.confirm(`确定删除「${selectedFile.value.name}」？此操作不可恢复。`)) return;
  try {
    const r = await ipc.sshFileDelete(
      props.connection.sessionId,
      selectedFile.value.path,
      selectedFile.value.isDir,
    );
    if (r.ok) {
      ui.toast(`已删除 ${selectedFile.value.name}`);
      selectedFile.value = null;
      navigate(currentPath.value);
    } else {
      ui.toast(`删除失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`删除失败：${e}`);
  }
}

async function rename() {
  if (!selectedFile.value || !props.connection?.sessionId) {
    if (!selectedFile.value) ui.toast("请先选择文件");
    return;
  }
  const name = window.prompt("新文件名：", selectedFile.value.name);
  if (!name || name === selectedFile.value.name) return;
  const dir = selectedFile.value.path.slice(0, selectedFile.value.path.lastIndexOf("/") + 1);
  const newPath = `${dir}${name}`;
  try {
    const r = await ipc.sshFileRename(props.connection.sessionId, selectedFile.value.path, newPath);
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
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 路径导航 + 操作栏 -->
    <div
      class="flex shrink-0 items-center gap-[8px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <button class="btn-ghost !px-[6px] !py-[3px] text-caption" title="上级目录" @click="navigateUp">
        ↑ 上级
      </button>
      <input
        v-model="currentPath"
        class="field-input !h-[28px] flex-1 !py-[4px] font-mono text-body-sm"
        spellcheck="false"
        @keyup.enter="navigate(currentPath)"
      />
      <button class="btn-secondary !px-[12px] text-body-sm" @click="upload">上传</button>
      <button class="btn-secondary !px-[12px] text-body-sm" @click="download">下载</button>
      <button class="btn-secondary !px-[12px] text-body-sm" @click="rename">重命名</button>
      <button
        class="btn-secondary !px-[12px] text-body-sm text-danger-strong dark:text-danger-dark"
        @click="del"
      >
        删除
      </button>
    </div>

    <!-- 文件列表 -->
    <div class="min-h-0 flex-1 overflow-y-auto">
      <table class="w-full text-left text-body">
        <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
          <tr class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark">
            <th class="px-[12px] py-[8px] font-medium">名称</th>
            <th class="w-[100px] px-[12px] py-[8px] font-medium">大小</th>
            <th class="w-[120px] px-[12px] py-[8px] font-medium">修改时间</th>
            <th class="w-[110px] px-[12px] py-[8px] font-medium">权限</th>
            <th class="w-[80px] px-[12px] py-[8px] font-medium">所有者</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="f in files"
            :key="f.path"
            class="cursor-pointer border-b border-border/50 transition-colors hover:bg-surface-muted dark:border-border-dark/50 dark:hover:bg-surface-muted-dark"
            :class="{
              'bg-tertiary-soft dark:bg-tertiary-soft-dark': selectedFile?.path === f.path,
            }"
            @click="selectedFile = f"
            @dblclick="onDoubleClick(f)"
          >
            <td class="px-[12px] py-[7px]">
              <span class="mr-[6px]">{{ f.isDir ? "📁" : "📄" }}</span>
              <span :class="{ 'font-medium': f.isDir }">{{ f.name }}</span>
            </td>
            <td class="px-[12px] py-[7px] font-mono text-body-sm">
              {{ f.isDir ? "-" : formatBytes(f.size) }}
            </td>
            <td class="px-[12px] py-[7px] text-body-sm">{{ formatTime(f.modifiedAt) }}</td>
            <td class="px-[12px] py-[7px] font-mono text-body-sm">{{ f.permissions }}</td>
            <td class="px-[12px] py-[7px] text-body-sm">{{ f.owner }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 状态栏 -->
    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>{{ currentPath }}</span>
      <span>{{ files.length }} 个项目</span>
      <span v-if="selectedFile" class="text-tertiary-strong dark:text-tertiary-dark">
        已选：{{ selectedFile.name }}
      </span>
    </div>

    <!-- 远程编辑弹窗（双击文件打开） -->
    <EditorDialog
      v-if="editing"
      :path="editing.path"
      :content="editing.content"
      @save="onSave"
      @cancel="editing = null"
    />
  </div>
</template>
