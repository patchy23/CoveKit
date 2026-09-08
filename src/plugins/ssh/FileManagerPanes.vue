<script setup lang="ts">
/**
 * FileManagerPanes · 文件页双栏区（远程 FileBrowser + 中列传输按钮 + 本地 LocalBrowser + 拖拽遮罩）
 * 从 FileManagerTab 拆出（300 行红线）；纯布局装配，事件全部上抛。
 */
import { ref } from 'vue'
import type { RemoteFile } from './contracts'
import { UiIconButton } from '@/core/ui'
import FileBrowser from './FileBrowser.vue'
import LocalBrowser from './LocalBrowser.vue'

defineProps<{
  dragActive: boolean
  currentPath: string
  files: RemoteFile[]
  active?: boolean
  canGoBack: boolean
  sortKey: 'name' | 'modifiedAt'
  sortDirection: 'asc' | 'desc'
  remoteSelectedPaths: Set<string>
  remoteSelectedCount: number
  transferStatus?: string
  localInitialPath: string
  localSelectedPaths: Set<string>
}>()

const emit = defineEmits<{
  (e: 'navigate', path: string): void
  (e: 'back'): void
  (e: 'up'): void
  (e: 'upload'): void
  (e: 'uploadDirectory'): void
  (e: 'download'): void
  (e: 'rename'): void
  (e: 'delete'): void
  (e: 'remoteRowClick', mouse: MouseEvent, file: RemoteFile): void
  (e: 'open', file: RemoteFile): void
  (e: 'remoteContext', mouse: MouseEvent, file: RemoteFile | null): void
  (e: 'sort', key: 'name' | 'modifiedAt'): void
  (e: 'uploadLocal'): void
  (e: 'localRowClick', mouse: MouseEvent, file: RemoteFile): void
  (e: 'localRowContext', mouse: MouseEvent, file: RemoteFile): void
  (e: 'localBlankContext', mouse: MouseEvent): void
  (e: 'localError', message: string): void
}>()

/** 本地浏览器实例（父级读取 files/currentPath/refresh 用，defineExpose 转发） */
const localBrowser = ref<InstanceType<typeof LocalBrowser> | null>(null)
defineExpose({ localBrowser })
</script>

<template>
  <div
    v-if="dragActive"
    class="pointer-events-none absolute inset-[8px] z-[60] flex items-center justify-center rounded-lg border-2 border-dashed border-tertiary bg-tertiary-soft/90 text-h2 text-tertiary-strong shadow-card dark:border-tertiary-dark dark:bg-tertiary-soft-dark/90 dark:text-tertiary-dark"
  >
    释放文件，上传到 {{ currentPath }}
  </div>
  <div class="flex min-h-0 flex-1">
    <FileBrowser
      class="min-w-[420px] flex-[3]"
      :current-path="currentPath"
      :files="files"
      :active="active"
      :can-go-back="canGoBack"
      :sort-key="sortKey"
      :sort-direction="sortDirection"
      :selected-paths="remoteSelectedPaths"
      :selected-count="remoteSelectedCount"
      :transfer-status="transferStatus"
      @navigate="(p: string) => emit('navigate', p)"
      @back="emit('back')"
      @up="emit('up')"
      @upload="emit('upload')"
      @upload-directory="emit('uploadDirectory')"
      @download="emit('download')"
      @rename="emit('rename')"
      @delete="emit('delete')"
      @row-click="(e: MouseEvent, f: RemoteFile) => emit('remoteRowClick', e, f)"
      @open="(f: RemoteFile) => emit('open', f)"
      @context="(e: MouseEvent, f: RemoteFile | null) => emit('remoteContext', e, f)"
      @sort="(k: 'name' | 'modifiedAt') => emit('sort', k)"
    />

    <!-- 中列传输按钮 -->
    <div
      class="flex w-[42px] shrink-0 flex-col items-center justify-center gap-[8px] border-l border-border dark:border-border-dark"
    >
      <UiIconButton
        label="上传到远端"
        title="把左侧选中的本地文件/目录上传到远端当前目录"
        @click="emit('uploadLocal')"
      >
        →
      </UiIconButton>
    </div>

    <!-- 右栏：本地目录 -->
    <LocalBrowser
      ref="localBrowser"
      class="min-w-[240px] flex-[2]"
      :initial-path="localInitialPath"
      :selected-paths="localSelectedPaths"
      @row-click="(e: MouseEvent, f: RemoteFile) => emit('localRowClick', e, f)"
      @row-context="(e: MouseEvent, f: RemoteFile) => emit('localRowContext', e, f)"
      @blank-context="(e: MouseEvent) => emit('localBlankContext', e)"
      @error="(m: string) => emit('localError', m)"
    />
  </div>
</template>
