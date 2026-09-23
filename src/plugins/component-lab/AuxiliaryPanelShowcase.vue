<script setup lang="ts">
import { ref } from 'vue'
import { UiPopover, UiBottomPanel, UiButton, UiInput, UiPanel, UiAlert } from '@/core/ui'
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
      <UiBottomPanel v-model:open="panel" title="任务与输出">
        <div class="flex flex-col gap-sm p-md">
          <p class="text-body-sm">
            拖动上边缘调整高度，也可以聚焦分隔条使用方向键；关闭面板保留内部状态。
          </p>
          <UiInput v-model="draft" />
        </div>
      </UiBottomPanel>
    </div>
  </UiPanel>
</template>
