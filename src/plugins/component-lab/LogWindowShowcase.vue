<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { UiButton, UiFloatingWindow, UiLogViewer, UiPanel } from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'

const { copyText } = useCopy()
const windows = ref<{ id: number; limit: number }[]>([])
const content = ref('')
let sequence = 0
let nextWindow = 0
let timer: ReturnType<typeof setInterval> | undefined
function append() {
  content.value = [
    ...content.value.split('\n').filter(Boolean),
    `${new Date().toLocaleTimeString()} INFO 请求 ${++sequence} 已完成 · 这是可选择复制的演示日志`,
  ]
    .slice(-2000)
    .join('\n')
}
onMounted(() => {
  append()
  timer = setInterval(append, 1000)
})
onUnmounted(() => clearInterval(timer))
</script>

<template>
  <UiPanel title="日志与浮动窗口">
    <div class="flex flex-col gap-sm">
      <p class="text-body-sm text-secondary dark:text-secondary-dark">
        可同时打开多个窗口，拖动标题栏、拖动右下角或最大化。暂停只冻结当前窗口的内容，后台仍每秒产生一行。
      </p>
      <div class="flex gap-sm">
        <UiButton size="sm" @click="windows.push({ id: ++nextWindow, limit: 300 })"
          >打开日志窗口</UiButton
        ><UiButton size="sm" variant="ghost" @click="windows = []">关闭全部演示窗口</UiButton>
      </div>
      <div
        class="relative isolate h-[620px] overflow-hidden rounded-md border border-border bg-neutral dark:border-border-dark dark:bg-neutral-dark"
      >
        <p class="p-md text-body-sm text-secondary dark:text-secondary-dark">
          窗口不会阻止操作背景；可打开两个窗口比较暂停与实时显示。
        </p>
        <UiFloatingWindow
          v-for="item in windows"
          :key="item.id"
          :title="`日志演示 ${item.id}`"
          :width="740"
          :height="400"
          @close="windows = windows.filter((win) => win.id !== item.id)"
        >
          <UiLogViewer
            :content="content.split('\n').slice(-item.limit).join('\n')"
            @limit-change="item.limit = $event"
            @refresh="append"
            @copy="copyText($event)"
          />
        </UiFloatingWindow>
      </div>
    </div>
  </UiPanel>
</template>
