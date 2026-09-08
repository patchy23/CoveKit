<script setup lang="ts">
/**
 * RemotePathToolbar · 远程文件工具栏（导航图标按钮 + 面包屑路径 + 操作图标按钮）
 * 文字按钮已全部图标化，保证路径区不被挤压。
 */
import { UiIcon, UiIconButton } from '@/core/ui'
import PathBreadcrumbs from './PathBreadcrumbs.vue'

defineProps<{ currentPath: string; canGoBack?: boolean }>()
const emit = defineEmits<{
  navigate: [path: string]
  back: []
  up: []
  upload: []
  uploadDirectory: []
  download: []
  rename: []
  delete: []
}>()
</script>

<template>
  <div
    class="flex shrink-0 items-center gap-[4px] border-b border-border px-[8px] py-[6px] dark:border-border-dark"
  >
    <UiIconButton
      label="后退"
      size="sm"
      title="返回上一次目录"
      :disabled="!canGoBack"
      @click="emit('back')"
    >
      <UiIcon name="arrow-left" :size="14" />
    </UiIconButton>
    <UiIconButton label="上级" size="sm" title="上级目录" @click="emit('up')">
      <UiIcon name="arrow-up" :size="14" />
    </UiIconButton>

    <PathBreadcrumbs
      class="min-w-0 flex-1"
      :path="currentPath"
      separator="/"
      @navigate="emit('navigate', $event)"
    />

    <UiIconButton label="上传文件" size="sm" title="上传文件" @click="emit('upload')">
      <UiIcon name="upload" :size="14" />
    </UiIconButton>
    <UiIconButton label="上传目录" size="sm" title="上传目录" @click="emit('uploadDirectory')">
      <UiIcon name="folder-up" :size="14" />
    </UiIconButton>
    <UiIconButton label="下载" size="sm" title="下载选中项" @click="emit('download')">
      <UiIcon name="download" :size="14" />
    </UiIconButton>
    <UiIconButton label="重命名" size="sm" title="重命名选中项" @click="emit('rename')">
      <UiIcon name="pencil" :size="14" />
    </UiIconButton>
    <UiIconButton
      label="删除"
      size="sm"
      title="删除选中项"
      class="text-danger-strong dark:text-danger-dark"
      @click="emit('delete')"
    >
      <UiIcon name="trash" :size="14" />
    </UiIconButton>
  </div>
</template>
