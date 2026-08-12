<script setup lang="ts">
import { ref } from 'vue'
import AppIcon from '@/features/ui/AppIcon.vue'
import {
  UiAlert,
  UiAvatar,
  UiBadge,
  UiButton,
  UiDivider,
  UiIconButton,
  UiKbd,
  UiPanel,
  UiSpinner,
} from '@/core/ui'
import type { UiSize } from '@/core/ui'

const sizes: UiSize[] = ['xs', 'sm', 'md', 'lg']
const loading = ref(false)

function previewLoading() {
  loading.value = true
  window.setTimeout(() => (loading.value = false), 1000)
}
</script>

<template>
  <UiPanel
    title="按钮与动作"
    description="四档尺寸覆盖表格行内动作、普通工具栏、表单主操作和强调入口。"
  >
    <div class="flex flex-col gap-md">
      <div v-for="size in sizes" :key="size" class="flex flex-wrap items-center gap-sm">
        <UiBadge size="xs">{{ size }}</UiBadge>
        <UiButton variant="primary" :size="size">主要操作</UiButton>
        <UiButton :size="size">次要操作</UiButton>
        <UiButton variant="ghost" :size="size">幽灵操作</UiButton>
        <UiButton variant="danger" :size="size">危险操作</UiButton>
        <UiIconButton label="复制" :size="size"><AppIcon name="copy" :size="14" /></UiIconButton>
      </div>
      <UiDivider />
      <div class="flex flex-wrap items-center gap-sm">
        <UiButton variant="primary" :loading="loading" @click="previewLoading">
          {{ loading ? '处理中…' : '加载状态' }}
        </UiButton>
        <UiButton disabled>禁用状态</UiButton>
        <UiButton block class="max-w-[220px]">块级按钮</UiButton>
        <UiSpinner size="sm">正在同步</UiSpinner>
      </div>
    </div>
  </UiPanel>

  <div class="grid gap-md lg:grid-cols-2">
    <UiPanel title="徽标与状态" description="尺寸和语义颜色彼此独立，紧凑列表默认使用 xs。">
      <div class="flex flex-col gap-md">
        <div v-for="size in sizes" :key="size" class="flex flex-wrap items-center gap-sm">
          <UiBadge :size="size">默认 {{ size }}</UiBadge>
          <UiBadge :size="size" tone="accent">强调</UiBadge>
          <UiBadge :size="size" tone="success">成功</UiBadge>
          <UiBadge :size="size" tone="warning">警告</UiBadge>
          <UiBadge :size="size" tone="danger">失败</UiBadge>
          <UiBadge :size="size" tone="info">信息</UiBadge>
        </div>
      </div>
    </UiPanel>

    <UiPanel title="头像与键盘提示" description="连接列表、成员信息和快捷键展示的通用元素。">
      <div class="flex items-end gap-sm">
        <UiAvatar v-for="size in sizes" :key="size" name="Patchy Box" :size="size" />
      </div>
      <div
        class="mt-lg flex items-center gap-xs text-body-sm text-secondary dark:text-secondary-dark"
      >
        快速搜索 <UiKbd>Ctrl</UiKbd><span>+</span><UiKbd>K</UiKbd>
      </div>
      <div class="mt-md rounded-md bg-surface-muted p-sm text-body-sm dark:bg-surface-muted-dark">
        <span class="text-text-muted dark:text-text-muted-dark">等宽中英混排：</span>
        <span class="font-mono text-danger-strong dark:text-danger-dark">
          api-server-01 · 连接失败
        </span>
      </div>
    </UiPanel>
  </div>

  <UiPanel title="行内反馈" description="提示框同样支持四档密度，业务只选择语义，不拼装颜色。">
    <div class="grid gap-sm lg:grid-cols-2">
      <UiAlert size="xs" tone="info">xs：适合表格上方的紧凑说明。</UiAlert>
      <UiAlert size="sm" tone="success">sm：配置已经保存成功。</UiAlert>
      <UiAlert size="md" tone="warning">md：当前操作可能需要管理员权限。</UiAlert>
      <UiAlert size="lg" tone="danger" title="连接失败">lg：请检查主机地址和认证信息。</UiAlert>
    </div>
  </UiPanel>
</template>
