<script setup lang="ts">
/**
 * FileManagerTab · 远程文件管理子页签
 * 路径导航 + 文件列表 + 上传/下载/删除/重命名操作（当前为假数据演示）
 */
import { computed, ref } from "vue";
import type { ServerConnection, ServerProfile, RemoteFile } from "./contracts";
import { formatBytes, formatTime, mockFiles } from "./useSsh";
import { useUiStore } from "@/stores/ui";

defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const currentPath = ref("/var/log/nginx");
const files = ref<RemoteFile[]>([...mockFiles]);
const selectedFile = ref<RemoteFile | null>(null);

const parentPath = computed(() => {
  const p = currentPath.value;
  if (p === "/") return null;
  const idx = p.lastIndexOf("/");
  return idx <= 0 ? "/" : p.slice(0, idx);
});

function navigate(path: string) {
  currentPath.value = path;
  selectedFile.value = null;
  // TODO: IPC 加载远程目录
}

function navigateUp() {
  if (parentPath.value) navigate(parentPath.value);
}

function onDoubleClick(file: RemoteFile) {
  if (file.isDir) {
    navigate(file.path);
  } else {
    ui.toast(`打开编辑：${file.name}（远程编辑页签开发中）`);
  }
}

function upload() {
  ui.toast("上传功能待后端 IPC 接入");
}

function download() {
  if (!selectedFile.value) {
    ui.toast("请先选择文件");
    return;
  }
  ui.toast(`下载 ${selectedFile.value.name}（待后端 IPC 接入）`);
}

function del() {
  if (!selectedFile.value) {
    ui.toast("请先选择文件");
    return;
  }
  ui.toast(`删除 ${selectedFile.value.name}（待后端 IPC 接入）`);
}

function rename() {
  if (!selectedFile.value) {
    ui.toast("请先选择文件");
    return;
  }
  ui.toast(`重命名 ${selectedFile.value.name}（待后端 IPC 接入）`);
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
      <button class="btn-secondary !h-[28px] !px-[10px] text-caption" @click="upload">上传</button>
      <button class="btn-secondary !h-[28px] !px-[10px] text-caption" @click="download">下载</button>
      <button class="btn-secondary !h-[28px] !px-[10px] text-caption" @click="rename">重命名</button>
      <button
        class="btn-secondary !h-[28px] !px-[10px] text-caption text-danger-strong dark:text-danger-dark"
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
  </div>
</template>
