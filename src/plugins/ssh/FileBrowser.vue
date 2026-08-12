<script setup lang="ts">
/** FileBrowser · SSH 文件页的路径工具栏与远程文件表格。 */
import type { RemoteFile } from './contracts'
import { formatBytes, formatTime } from './useSsh'
import { UiButton, UiInput, UiTableCell } from '@/core/ui'

defineProps<{
  currentPath: string
  files: RemoteFile[]
  selectedPath?: string
  selectedName?: string
  transferStatus?: string
}>()

const emit = defineEmits<{
  (event: 'update:currentPath', value: string): void
  (event: 'navigate', path: string): void
  (event: 'up'): void
  (event: 'upload'): void
  (event: 'download'): void
  (event: 'rename'): void
  (event: 'delete'): void
  (event: 'select', file: RemoteFile): void
  (event: 'open', file: RemoteFile): void
  (event: 'context', mouse: MouseEvent, file: RemoteFile | null): void
}>()
</script>

<template>
  <div
    class="flex shrink-0 items-center gap-[8px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
  >
    <UiButton variant="ghost" size="sm" title="上级目录" @click="emit('up')"> ↑ 上级 </UiButton>
    <UiInput
      :model-value="currentPath"
      size="sm"
      class="flex-1 font-mono"
      spellcheck="false"
      @update:model-value="emit('update:currentPath', String($event))"
      @keyup.enter="emit('navigate', currentPath)"
    />
    <UiButton size="sm" @click="emit('upload')">上传</UiButton>
    <UiButton size="sm" @click="emit('download')">下载</UiButton>
    <UiButton size="sm" @click="emit('rename')">重命名</UiButton>
    <UiButton size="sm" class="text-danger-strong dark:text-danger-dark" @click="emit('delete')">
      删除
    </UiButton>
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto" @contextmenu="emit('context', $event, null)">
    <table class="w-full text-left text-body-sm text-secondary dark:text-secondary-dark">
      <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
        <tr
          class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
        >
          <UiTableCell as="th" class="px-[12px] py-[8px]">名称</UiTableCell>
          <UiTableCell as="th" class="w-[100px] px-[12px] py-[8px]">大小</UiTableCell>
          <UiTableCell as="th" class="w-[120px] px-[12px] py-[8px]">修改时间</UiTableCell>
          <UiTableCell as="th" class="w-[110px] px-[12px] py-[8px]">权限</UiTableCell>
          <UiTableCell as="th" class="w-[80px] px-[12px] py-[8px]">所有者</UiTableCell>
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
          <UiTableCell content="technical" class="px-[12px] py-[7px]">
            <span class="mr-[6px]">{{ file.isDir ? '📁' : '📄' }}</span>
            <span :class="{ 'font-medium': file.isDir }">{{ file.name }}</span>
          </UiTableCell>
          <UiTableCell content="numeric" class="px-[12px] py-[7px]">
            {{ file.isDir ? '-' : formatBytes(file.size) }}
          </UiTableCell>
          <UiTableCell content="numeric" class="px-[12px] py-[7px]">{{
            formatTime(file.modifiedAt)
          }}</UiTableCell>
          <UiTableCell content="technical" class="px-[12px] py-[7px]">{{
            file.permissions
          }}</UiTableCell>
          <UiTableCell content="technical" class="px-[12px] py-[7px]">{{ file.owner }}</UiTableCell>
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
