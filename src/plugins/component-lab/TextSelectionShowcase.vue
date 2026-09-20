<script setup lang="ts">
import {
  UiAlert,
  UiButton,
  UiEmptyState,
  UiField,
  UiInput,
  UiPanel,
  UiStatusBar,
  UiTooltip,
} from '@/core/ui'
import { useUiStore } from '@/stores/ui'
const ui = useUiStore()
</script>

<template>
  <UiPanel
    title="文本选择与错误复制"
    description="正文可拖选复制，按钮、字段标签与悬停提示不应被误选。"
  >
    <div class="flex flex-col gap-md">
      <UiAlert tone="danger" title="连接失败"
        >ECONNREFUSED · 127.0.0.1:5432，请检查服务状态。<UiButton size="xs" variant="ghost"
          >重试按钮不可选</UiButton
        ></UiAlert
      >
      <UiField label="项目目录" error="EACCES：无法写入 /srv/example/compose.yml"
        ><UiInput model-value="/srv/example"
      /></UiField>
      <UiEmptyState
        compact
        selectable
        title="读取失败"
        description="ENOENT：找不到 /srv/example/config.yml"
      />
      <UiStatusBar
        ><span>当前路径：</span
        ><span class="select-text font-mono">/srv/example/config.yml</span></UiStatusBar
      >
      <div class="flex gap-sm">
        <UiButton
          size="sm"
          @click="ui.toast('请求失败：ETIMEDOUT\n目标：127.0.0.1:5432\n请检查服务状态和连接配置。')"
          >演示可保留的消息详情</UiButton
        >
        <UiTooltip content="操作说明保持不可选中，且鼠标穿透"
          ><UiButton size="sm" variant="ghost">悬停提示</UiButton></UiTooltip
        >
      </div>
    </div>
  </UiPanel>
</template>
