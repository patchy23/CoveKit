<script setup lang="ts">
import { ref } from 'vue'
import {
  UiPopover,
  UiBottomPanel,
  UiButton,
  UiInput,
  UiPanel,
  UiAlert,
  UiStatusBar,
  UiSearchInput,
  UiIconButton,
  UiIcon,
  UiSplitPane,
} from '@/core/ui'
const splitWidth = ref(180),
  resizable = ref(true)
const footerOpen = ref(false),
  footerQuery = ref(''),
  samples = ref(['项目目录', '运行日志'])
const popover = ref(false),
  panel = ref(true),
  draft = ref('收起后保留这段输入')
</script>
<template>
  <UiPanel title="辅助面板 · 点击弹层与底部任务区">
    <div class="flex h-[400px] min-h-0 flex-col">
      <div class="flex flex-1 items-start gap-sm p-md">
        <UiPopover v-model:open="popover" label="可交互弹层">
          <template #trigger><UiButton variant="ghost">打开弹层</UiButton></template>
          <div class="flex flex-col gap-sm p-md">
            <UiAlert tone="info"
              >弹层挂载到独立容器，自动避让窗口边界；点击外部或 Esc 关闭。</UiAlert
            ><UiInput placeholder="支持键盘输入" /><UiButton @click="popover = false"
              >完成</UiButton
            >
          </div>
        </UiPopover>
        <UiButton variant="ghost" @click="panel = !panel"
          >{{ panel ? '收起' : '展开' }}底部面板</UiButton
        >
      </div>
      <div class="flex items-center gap-sm px-md">
        <UiButton size="xs" variant="ghost" @click="resizable = !resizable">{{
          resizable ? '锁定分栏' : '允许调整分栏'
        }}</UiButton>
      </div>
      <UiSplitPane
        v-model="splitWidth"
        :resizable="resizable"
        :min="120"
        :max="260"
        class="h-[64px] shrink-0"
      >
        <template #primary><p class="p-sm text-caption">主内容区</p></template>
        <template #secondary
          ><p class="p-sm text-caption">锁定时保留内容，禁用分隔条焦点和拖拽。</p></template
        >
      </UiSplitPane>
      <UiBottomPanel v-model:open="panel" title="任务与输出">
        <div class="flex flex-col gap-sm p-md">
          <p class="text-body-sm">
            拖动上边缘调整高度，也可以聚焦分隔条使用方向键；关闭面板保留内部状态。
          </p>
          <UiInput v-model="draft" />
        </div>
      </UiBottomPanel>
      <UiStatusBar size="md">
        <span>底部操作浮层</span>
        <template #trailing>
          <UiPopover v-model:open="footerOpen" label="紧凑书签示例" side="top" width="320px">
            <template #trigger><UiButton size="sm" variant="ghost">书签</UiButton></template>
            <div class="p-sm">
              <UiSearchInput v-model="footerQuery" size="sm" placeholder="搜索书签" />
            </div>
            <div
              v-for="item in samples.filter((v) => v.includes(footerQuery))"
              :key="item"
              class="flex items-center gap-sm px-sm py-xs"
            >
              <span class="min-w-0 flex-1 truncate text-body-sm">{{ item }}</span>
              <UiIconButton
                size="xs"
                label="删除书签"
                @click.stop="samples = samples.filter((v) => v !== item)"
                ><UiIcon name="x" :size="12"
              /></UiIconButton>
            </div>
            <p
              v-if="!samples.length"
              class="px-sm py-xs text-caption text-secondary dark:text-secondary-dark"
            >
              暂无书签
            </p>
          </UiPopover>
        </template>
      </UiStatusBar>
    </div>
  </UiPanel>
</template>
