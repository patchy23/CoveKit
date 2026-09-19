<script setup lang="ts">
import { ref } from 'vue'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import {
  UiAlert,
  UiButton,
  UiEmptyState,
  UiInput,
  UiScrollArea,
  UiTable,
  UiTableCell,
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
      <div class="flex items-baseline gap-sm">
        <h1 class="text-h1 text-primary dark:text-primary-dark">文件占用查询</h1>
        <span class="text-body-sm text-secondary dark:text-secondary-dark">Windows</span>
      </div>
      <p class="text-body-sm text-secondary dark:text-secondary-dark">
        选择或拖入单个文件，查找正在使用它的进程。
      </p>
      <form class="flex flex-wrap items-center gap-sm" @submit.prevent="query">
        <UiInput
          v-model="path"
          aria-label="文件完整路径"
          placeholder="输入文件完整路径，或拖入一个文件"
          class="min-w-0 flex-1 basis-64 font-mono"
          :disabled="!available || picking"
        />
        <UiButton :disabled="!available || busy" :loading="picking" @click="chooseFile"
          >选择文件</UiButton
        >
        <UiButton
          type="submit"
          variant="primary"
          :disabled="!available || picking || !path.trim()"
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
            <p
              class="mb-md break-all font-mono text-body-sm text-secondary select-text dark:text-secondary-dark"
            >
              {{ result.path }}
            </p>
            <UiEmptyState
              v-if="result.processes.length === 0"
              title="未发现占用进程"
              description="本次查询未发现使用者，不保证文件没有占用。关闭相关程序后可以再次查询。"
            />
            <UiTable v-else>
              <thead>
                <tr>
                  <UiTableCell as="th">进程 / PID</UiTableCell>
                  <UiTableCell as="th">应用 / 服务</UiTableCell>
                  <UiTableCell as="th">程序路径 / 状态</UiTableCell>
                  <UiTableCell as="th" align="right" :resizable="false">操作</UiTableCell>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="process in result.processes"
                  :key="`${process.pid}:${process.startedAt}:${process.serviceName ?? ''}`"
                >
                  <UiTableCell content="technical" class="align-top">
                    <p class="break-all text-primary dark:text-primary-dark">
                      {{ process.processName ?? '进程名称未读取' }}
                    </p>
                    <p class="mt-xs whitespace-nowrap">PID {{ process.pid }}</p>
                  </UiTableCell>
                  <UiTableCell class="align-top">
                    <p class="break-all">{{ process.appName || '应用名称未提供' }}</p>
                    <p v-if="process.serviceName" class="mt-xs break-all">
                      服务：{{ process.serviceName }}
                    </p>
                  </UiTableCell>
                  <UiTableCell content="technical" class="max-w-lg align-top">
                    <p v-if="process.executablePath" class="break-all">
                      {{ process.executablePath }}
                    </p>
                    <p
                      v-if="process.detailError"
                      class="break-words text-warning-strong dark:text-warning-dark"
                    >
                      {{ process.detailError }}
                    </p>
                    <p v-else class="mt-xs text-secondary dark:text-secondary-dark">
                      进程信息已读取
                    </p>
                  </UiTableCell>
                  <UiTableCell content="action" align="right" class="align-top">
                    <div class="flex flex-wrap justify-end gap-xs">
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
    <div
      v-if="dragging"
      class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center border-2 border-dashed border-tertiary-strong bg-tertiary-soft text-h2 text-tertiary-strong dark:border-tertiary-dark dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
    >
      松开以查询这个文件
    </div>
  </section>
</template>
