<script setup lang="ts">
/**
 * RemotePathToolbar · 远程文件工具栏（后退/上级导航图标按钮 + 面包屑路径）
 * 上传/下载/重命名/删除不在工具栏放按钮：统一走列表右键菜单 + 双栏拖拽 + 中缝传输按钮。
 */
import { UiIcon, UiIconButton } from '@/core/ui'
import PathBreadcrumbs from './PathBreadcrumbs.vue'

defineProps<{ currentPath: string; canGoBack?: boolean }>()
const emit = defineEmits<{
  navigate: [path: string]
  back: []
  up: []
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
  </div>
</template>
