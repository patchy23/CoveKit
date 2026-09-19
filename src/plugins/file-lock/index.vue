<script setup lang="ts">
import { ref } from 'vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import {
  UiAlert,
  UiButton,
  UiConfirmDialog,
  UiEmptyState,
  UiInput,
  UiScrollArea,
  UiTable,
  UiTableCell,
  UiTooltip,
} from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { useUiStore } from '@/stores/ui'
import type { FileProcess } from './contracts'
import { useFileLock } from './useFileLock'

const page = ref<HTMLElement | null>(null)
const {
  desktop,
  supported,
  checking,
  available,
  path,
  busy,
  picking,
  error,
  dropError,
  dragging,
  result,
  queriedAt,
  query,
  chooseFile,
  initialize,
  closeTarget,
  closing,
  closeError,
  requestClose,
  cancelClose,
  confirmClose,
} = useFileLock(page)
const { copyText } = useCopy()
const ui = useUiStore()

function copyProcess(process: FileProcess) {
  void copyText(
    [
      `查询文件：${result.value?.path ?? ''}`,
      `进程：${process.processName ?? '未读取'}`,
      `PID：${process.pid}`,
      `应用：${process.appName || '未提供'}`,
      `服务：${process.serviceName ?? '无'}`,
      `程序路径：${process.executablePath ?? '未读取'}`,
      `状态：${process.detailError ?? '已读取'}`,
    ].join('\n'),
    '已复制进程信息'
  )
}

async function revealProgram(process: FileProcess) {
  if (!process.executablePath) return
  try {
    await revealItemInDir(process.executablePath)
  } catch (cause) {
    ui.toast(`打开程序所在目录失败：${String(cause)}`)
  }
}
</script>

<template>
  <section ref="page" class="relative flex h-full min-h-0 flex-col bg-surface dark:bg-surface-dark">
    <div class="space-y-md border-b border-border p-lg dark:border-border-dark">
      <form class="flex flex-wrap items-center gap-sm" @submit.prevent="query">
        <UiInput
          v-model="path"
          aria-label="文件完整路径"
          placeholder="输入文件完整路径，或拖入一个文件"
          class="min-w-0 flex-1 basis-64 font-mono"
          :disabled="!available || picking || !!closeTarget"
        />
        <UiButton
          :disabled="!available || busy || !!closeTarget"
          :loading="picking"
          @click="chooseFile"
          >选择文件</UiButton
        >
        <UiButton
          type="submit"
          variant="primary"
          :disabled="!available || picking || !!closeTarget || !path.trim()"
          :loading="busy"
        >
          {{ result ? '刷新查询' : '查询占用' }}
        </UiButton>
      </form>
      <UiAlert v-if="error" tone="danger" title="查询未完成">{{ error }}</UiAlert>
      <UiAlert v-if="dropError" tone="warning">{{ dropError }}</UiAlert>
    </div>

    <UiScrollArea class="min-h-0 flex-1">
      <div class="p-lg">
        <UiEmptyState
          v-if="!desktop"
          title="请在 CoveKit 桌面应用中使用"
          description="浏览器预览无法访问本机文件占用信息。"
        />
        <UiEmptyState
          v-else-if="supported === false"
          title="目前仅支持 Windows"
          description="当前平台暂不支持文件占用查询。"
        />
        <UiEmptyState
          v-else-if="supported === null"
          :title="checking ? '正在检查平台支持…' : '无法确认平台支持状态'"
        >
          <UiButton v-if="!checking" @click="initialize">重试</UiButton>
        </UiEmptyState>
        <div v-else aria-live="polite" :aria-busy="busy">
          <UiEmptyState
            v-if="busy"
            title="正在查询文件使用进程…"
            description="系统查询可能需要一些时间，请稍候。"
          />
          <template v-else-if="result">
            <div class="mb-md flex flex-wrap items-baseline justify-between gap-sm">
              <h2 class="text-h2 text-primary dark:text-primary-dark">
                使用该文件的进程 · {{ result.processes.length }}
              </h2>
              <span class="text-body-sm text-secondary dark:text-secondary-dark"
                >查询时间 {{ queriedAt }}</span
              >
            </div>
            <UiEmptyState
              v-if="result.processes.length === 0"
              title="未发现占用进程"
              description="本次查询未发现使用者，不保证文件没有占用。关闭相关程序后可以再次查询。"
            />
            <UiTable v-else table-class="table-fixed min-w-[820px]">
              <thead>
                <tr>
                  <UiTableCell as="th" class="w-[148px]">进程 / PID</UiTableCell>
                  <UiTableCell as="th" class="w-[148px]">应用 / 服务</UiTableCell>
                  <UiTableCell as="th">程序路径 / 状态</UiTableCell>
                  <UiTableCell as="th" align="right" :resizable="false" class="w-[300px]"
                    >操作</UiTableCell
                  >
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="process in result.processes"
                  :key="`${process.pid}:${process.startedAt}:${process.serviceName ?? ''}`"
                >
                  <UiTableCell content="technical" class="align-top">
                    <UiTooltip :content="process.processName ?? ''"
                      ><p class="truncate text-primary dark:text-primary-dark">
                        {{ process.processName ?? '进程名称未读取' }}
                      </p></UiTooltip
                    >
                    <p class="mt-xs whitespace-nowrap">PID {{ process.pid }}</p>
                  </UiTableCell>
                  <UiTableCell class="align-top">
                    <UiTooltip :content="process.appName"
                      ><p class="truncate">
                        {{ process.appName || '应用名称未提供' }}
                      </p></UiTooltip
                    >
                    <UiTooltip v-if="process.serviceName" :content="process.serviceName"
                      ><p class="mt-xs truncate">服务：{{ process.serviceName }}</p></UiTooltip
                    >
                  </UiTableCell>
                  <UiTableCell content="technical" class="align-top">
                    <UiTooltip v-if="process.executablePath" :content="process.executablePath"
                      ><p class="truncate">
                        {{ process.executablePath }}
                      </p></UiTooltip
                    >
                    <UiTooltip v-if="process.detailError" :content="process.detailError"
                      ><p class="truncate text-warning-strong dark:text-warning-dark">
                        {{ process.detailError }}
                      </p></UiTooltip
                    >
                  </UiTableCell>
                  <UiTableCell content="action" align="right" class="align-top">
                    <div class="flex justify-end gap-xs whitespace-nowrap">
                      <UiButton size="sm" variant="ghost" @click="copyProcess(process)"
                        >复制信息</UiButton
                      >
                      <UiButton
                        size="sm"
                        variant="ghost"
                        :disabled="!process.executablePath"
                        @click="revealProgram(process)"
                        >打开所在目录</UiButton
                      >
                      <UiButton
                        size="sm"
                        variant="ghost"
                        class="text-danger-strong dark:text-danger-dark"
                        :disabled="closing"
                        @click="requestClose(process)"
                      >
                        关闭进程
                      </UiButton>
                    </div>
                  </UiTableCell>
                </tr>
              </tbody>
            </UiTable>
            <p class="mt-md text-body-sm text-secondary dark:text-secondary-dark">
              结果为查询时的进程快照。使用文件不一定阻止删除；关闭相关程序后请刷新确认。
            </p>
          </template>
          <UiEmptyState
            v-else-if="!error"
            title="选择要查询的文件"
            description="遇到无法删除或重命名的文件时，可在这里定位相关程序。暂不支持目录和批量查询。"
          />
        </div>
      </div>
    </UiScrollArea>
    <UiConfirmDialog
      :open="!!closeTarget"
      title="关闭进程"
      :message="`确定强制关闭 ${closeTarget?.process.processName || closeTarget?.process.appName || '所选进程'}（PID ${closeTarget?.process.pid ?? ''}）？该进程的所有窗口和任务都会结束，未保存的内容可能丢失。`"
      confirm-label="确认关闭"
      danger
      :loading="closing"
      :error="closeError"
      @confirm="confirmClose"
      @close="cancelClose"
    />
    <div
      v-if="dragging"
      class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center border-2 border-dashed border-tertiary-strong bg-tertiary-soft text-h2 text-tertiary-strong dark:border-tertiary-dark dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
    >
      松开以查询这个文件
    </div>
  </section>
</template>
