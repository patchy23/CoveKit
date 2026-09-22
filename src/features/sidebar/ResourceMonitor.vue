<script setup lang="ts">
/** 侧栏资源入口：只读详情不影响后台业务，所有读数来自应用级采样会话。 */
import { computed } from 'vue'
import { UiButton, UiHoverDetails, UiPanel, UiScrollArea } from '@/core/ui'
import { memoryLabel } from '@/core/resourceMonitor/sampling'
import { useResourceMonitorStore } from '@/stores/resourceMonitor'
import { useUiStore } from '@/stores/ui'
const monitor = useResourceMonitorStore()
const ui = useUiStore()
const cpu = computed(() =>
  monitor.totals?.cpu == null ? '—' : `${monitor.totals.cpu.toFixed(1)}%`
)
const overview = computed(() => [
  {
    label: '内存',
    detailLabel:
      monitor.snapshot?.value.memoryMetric === 'rss' ? '内存 · RSS' : '内存 · 私有工作集',
    value:
      monitor.totals && monitor.totals.memory === null
        ? '不可用'
        : memoryLabel(monitor.totals?.memory),
    peak: monitor.totals ? memoryLabel(monitor.peakMemory) : '—',
  },
  {
    label: 'CPU',
    detailLabel: 'CPU',
    value: cpu.value,
    peak: monitor.peakCpu == null ? '—' : `${monitor.peakCpu.toFixed(1)}%`,
  },
])
const memoryRows = computed(() => [
  ['主进程内存', memoryLabel(monitor.totals?.main)],
  [
    '界面进程内存',
    monitor.snapshot?.value.processes.some((p) => p.kind === 'webview')
      ? memoryLabel(monitor.totals?.webview)
      : '未覆盖',
  ],
])
const extraRows = computed(() => [
  ['其他子进程内存', memoryLabel(monitor.totals?.child)],
  ['完整工作集合计', memoryLabel(monitor.totals?.resident)],
  [
    '私有提交合计',
    monitor.totals?.privateBytes == null ? '不可用' : memoryLabel(monitor.totals.privateBytes),
  ],
])
const counts = computed(() => [
  ['进程', monitor.totals?.processes ?? '—'],
  ['线程', monitor.totals ? (monitor.totals.threads ?? '不可用') : '—'],
  ['句柄', monitor.totals ? (monitor.totals.handles ?? '不可用') : '—'],
])
/** 关闭且没有在途请求或残留资源时收紧展示，历史累计数仍保存在监测会话中。 */
function showToolDetails(tool: (typeof monitor.details)[number]) {
  return (
    ui.openTabs.includes(tool.id) ||
    tool.inFlight > 0 ||
    tool.scopes > 0 ||
    tool.listeners > 0 ||
    tool.timers > 0
  )
}
function openSettings(close: () => void) {
  close()
  monitor.focusSettings = true
  ui.openSettings()
}
</script>

<template>
  <UiHoverDetails v-if="monitor.enabled" label="应用资源详情">
    <template #trigger>
      <span class="grid w-full grid-cols-2 gap-x-md text-left">
        <span v-for="metric in overview" :key="metric.label" class="min-w-0">
          <span class="block text-body-sm font-normal text-secondary dark:text-secondary-dark">{{
            metric.label
          }}</span>
          <span
            class="mt-xs block whitespace-nowrap font-mono text-body font-semibold tabular-nums text-primary dark:text-primary-dark"
            >{{ metric.value }}</span
          >
        </span>
        <span
          v-if="monitor.error"
          class="col-span-2 text-left text-warning-strong dark:text-warning-dark"
          >更新中断</span
        >
        <span
          v-else-if="monitor.snapshot?.value.missingProcesses"
          class="col-span-2 text-left text-warning-strong dark:text-warning-dark"
          >部分进程不可读</span
        >
        <span
          v-else-if="monitor.snapshot?.value.partial"
          class="col-span-2 text-left text-secondary dark:text-secondary-dark"
          >仅部分进程</span
        >
      </span>
    </template>
    <template #default="{ close }">
      <p class="text-h2 text-primary dark:text-primary-dark">应用资源</p>
      <UiScrollArea class="mt-sm max-h-[min(65vh,520px)]" axis="vertical">
        <div class="pr-xs text-body-sm">
          <p
            v-if="monitor.error"
            class="select-text mb-sm text-warning-strong dark:text-warning-dark"
          >
            {{ monitor.error }}
          </p>
          <p v-else-if="!monitor.totals" class="mb-sm text-secondary dark:text-secondary-dark">
            正在采样…
          </p>
          <div class="select-text grid grid-cols-2 gap-md py-xs">
            <div v-for="metric in overview" :key="metric.label" class="min-w-0">
              <p class="text-secondary dark:text-secondary-dark">{{ metric.detailLabel }}</p>
              <p
                class="mt-xs whitespace-nowrap font-mono text-h1 font-semibold tabular-nums text-primary dark:text-primary-dark"
              >
                {{ metric.value }}
              </p>
              <p class="mt-xs text-caption text-secondary dark:text-secondary-dark">
                峰值 {{ metric.peak }}
              </p>
            </div>
          </div>
          <dl class="select-text mt-md grid grid-cols-[1fr_auto] gap-x-md gap-y-xs">
            <template v-for="[label, value] in memoryRows" :key="label">
              <dt class="text-secondary dark:text-secondary-dark">{{ label }}</dt>
              <dd class="text-right font-mono tabular-nums text-primary dark:text-primary-dark">
                {{ value }}
              </dd>
            </template>
          </dl>
          <dl
            class="select-text mt-sm grid grid-cols-3 gap-sm border-t border-border pt-sm dark:border-border-dark"
          >
            <div v-for="[label, value] in counts" :key="label">
              <dt class="text-caption text-secondary dark:text-secondary-dark">{{ label }}</dt>
              <dd class="mt-xs font-mono tabular-nums text-primary dark:text-primary-dark">
                {{ value }}
              </dd>
            </div>
          </dl>
          <UiPanel
            collapsible
            :default-open="false"
            padding="none"
            class="mt-sm !rounded-none !border-0 !bg-transparent"
          >
            <template #header
              ><span class="text-body-sm text-secondary dark:text-secondary-dark"
                >更多指标</span
              ></template
            >
            <dl class="select-text grid grid-cols-[1fr_auto] gap-x-md gap-y-xs">
              <template v-for="[label, value] in extraRows" :key="label">
                <dt class="text-secondary dark:text-secondary-dark">{{ label }}</dt>
                <dd class="text-right font-mono tabular-nums text-primary dark:text-primary-dark">
                  {{ value }}
                </dd>
              </template>
            </dl>
          </UiPanel>
          <p
            v-if="monitor.snapshot?.value.partial"
            class="mt-sm text-caption text-secondary dark:text-secondary-dark"
          >
            部分界面进程未计入
          </p>
          <p
            v-if="monitor.snapshot?.value.missingProcesses"
            class="mt-xs text-warning-strong dark:text-warning-dark"
          >
            {{ monitor.snapshot.value.missingProcesses }} 个进程未计入
          </p>
          <div
            v-if="monitor.selected.length || monitor.catalogError"
            class="mt-md border-t border-border pt-sm dark:border-border-dark"
          >
            <p class="font-semibold text-primary dark:text-primary-dark">工具统计</p>
            <p
              v-if="monitor.catalogError"
              class="select-text mt-xs text-warning-strong dark:text-warning-dark"
            >
              {{ monitor.catalogError }}
            </p>
            <div v-for="tool in monitor.details" :key="tool.id" class="mt-sm" :data-tool="tool.id">
              <p class="flex justify-between gap-sm text-primary dark:text-primary-dark">
                <span>{{
                  monitor.supportedTools.find((item) => item.id === tool.id)?.name ?? tool.id
                }}</span>
                <span class="text-caption text-secondary dark:text-secondary-dark">{{
                  ui.openTabs.includes(tool.id) ? '已打开' : '未打开'
                }}</span>
              </p>
              <div
                v-if="showToolDetails(tool)"
                class="select-text mt-xs text-secondary dark:text-secondary-dark"
              >
                <p>
                  请求 {{ tool.requests }} · 在途 {{ tool.inFlight }} · 失败 {{ tool.failures }}
                </p>
                <p class="select-text text-secondary dark:text-secondary-dark">
                  平均响应
                  {{ tool.completed ? `${Math.round(tool.totalMs / tool.completed)} ms` : '—' }}
                </p>
                <p class="select-text text-secondary dark:text-secondary-dark">
                  作用域 {{ tool.scopes }} · 订阅 {{ tool.listeners }} · 定时器 {{ tool.timers }}
                </p>
              </div>
            </div>
          </div>
        </div>
      </UiScrollArea>
      <div
        class="mt-sm flex items-center justify-between gap-sm border-t border-border pt-sm dark:border-border-dark"
      >
        <span class="text-caption text-secondary dark:text-secondary-dark">{{
          monitor.updatedAt
            ? `上次成功 ${new Date(monitor.updatedAt).toLocaleTimeString()}`
            : '尚无成功样本'
        }}</span>
        <UiButton size="xs" variant="ghost" @click="openSettings(close)">监测设置</UiButton>
      </div>
    </template>
  </UiHoverDetails>
</template>
