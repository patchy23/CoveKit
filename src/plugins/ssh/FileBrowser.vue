<script setup lang="ts">
/** FileBrowser · SSH 文件页的路径工具栏与远程文件表格。 */
import type { RemoteFile } from "./contracts";
import { formatBytes, formatTime } from "./useSsh";

defineProps<{
  currentPath: string;
  files: RemoteFile[];
  selectedPath?: string;
  selectedName?: string;
  transferStatus?: string;
}>();

const emit = defineEmits<{
  (event: "update:currentPath", value: string): void;
  (event: "navigate", path: string): void;
  (event: "up"): void;
  (event: "upload"): void;
  (event: "download"): void;
  (event: "rename"): void;
  (event: "delete"): void;
  (event: "select", file: RemoteFile): void;
  (event: "open", file: RemoteFile): void;
  (event: "context", mouse: MouseEvent, file: RemoteFile | null): void;
}>();
</script>

<template>
  <div
    class="flex shrink-0 items-center gap-[8px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
  >
    <button class="btn-ghost !px-[6px] !py-[3px] text-caption" title="上级目录" @click="emit('up')">
      ↑ 上级
    </button>
    <input
      :value="currentPath"
      class="field-input !h-[28px] flex-1 !py-[4px] font-mono text-body-sm"
      spellcheck="false"
      @input="emit('update:currentPath', ($event.target as HTMLInputElement).value)"
      @keyup.enter="emit('navigate', ($event.target as HTMLInputElement).value)"
    />
    <button class="btn-secondary !px-[12px] text-body-sm" @click="emit('upload')">上传</button>
    <button class="btn-secondary !px-[12px] text-body-sm" @click="emit('download')">下载</button>
    <button class="btn-secondary !px-[12px] text-body-sm" @click="emit('rename')">重命名</button>
    <button
      class="btn-secondary !px-[12px] text-body-sm text-danger-strong dark:text-danger-dark"
      @click="emit('delete')"
    >
      删除
    </button>
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto" @contextmenu="emit('context', $event, null)">
    <table class="w-full text-left text-body">
      <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
        <tr
          class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
        >
          <th class="px-[12px] py-[8px] font-medium">名称</th>
          <th class="w-[100px] px-[12px] py-[8px] font-medium">大小</th>
          <th class="w-[120px] px-[12px] py-[8px] font-medium">修改时间</th>
          <th class="w-[110px] px-[12px] py-[8px] font-medium">权限</th>
          <th class="w-[80px] px-[12px] py-[8px] font-medium">所有者</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="file in files"
          :key="file.path"
          class="cursor-pointer border-b border-border/50 transition-colors hover:bg-surface-muted dark:border-border-dark/50 dark:hover:bg-surface-muted-dark"
          :class="{ 'bg-tertiary-soft dark:bg-tertiary-soft-dark': selectedPath === file.path }"
          @click="emit('select', file)"
          @dblclick="emit('open', file)"
          @contextmenu.stop="emit('context', $event, file)"
        >
          <td class="px-[12px] py-[7px]">
            <span class="mr-[6px]">{{ file.isDir ? "📁" : "📄" }}</span>
            <span :class="{ 'font-medium': file.isDir }">{{ file.name }}</span>
          </td>
          <td class="px-[12px] py-[7px] font-mono text-body-sm">
            {{ file.isDir ? "-" : formatBytes(file.size) }}
          </td>
          <td class="px-[12px] py-[7px] text-body-sm">{{ formatTime(file.modifiedAt) }}</td>
          <td class="px-[12px] py-[7px] font-mono text-body-sm">{{ file.permissions }}</td>
          <td class="px-[12px] py-[7px] text-body-sm">{{ file.owner }}</td>
        </tr>
      </tbody>
    </table>
  </div>

  <div
    class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
  >
    <span>{{ currentPath }}</span>
    <span>{{ files.length }} 个项目</span>
    <span v-if="transferStatus" class="text-tertiary-strong dark:text-tertiary-dark">
      {{ transferStatus }}
    </span>
    <span v-if="selectedName" class="text-tertiary-strong dark:text-tertiary-dark">
      已选：{{ selectedName }}
    </span>
  </div>
</template>
