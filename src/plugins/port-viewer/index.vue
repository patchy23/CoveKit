<script setup lang="ts">
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import {
  UiAlert,
  UiButton,
  UiConfirmDialog,
  UiEmptyState,
  UiInput,
  UiPagination,
  UiScrollArea,
  UiSelect,
  UiSwitch,
  UiTable,
  UiTableCell,
  UiTooltip,
} from '@/core/ui'
import { useCopy } from '@/core/feedback/useCopy'
import { useUiStore } from '@/stores/ui'
import type { PortEntry } from './contracts'
import { addressText, copyEntry, entryKey } from './entries'
import { usePortViewer } from './usePortViewer'

const {
  desktop,
  supported,
  available,
  checking,
  busy,
  auto,
  snapshot,
  error,
  notice,
  queriedAt,
  filter,
  validation,
  filtered,
  page,
  totalPages,
  pageEntries,
  visible,
  closeTarget,
  closeError,
  closing,
  query,
  initialize,
  requestClose,
  cancelClose,
  confirmClose,
} = usePortViewer()
const { copyText } = useCopy()
const ui = useUiStore()
const views = [
  { value: 'listeners', label: '监听与绑定' },
  { value: 'all', label: '全部连接' },
]
const protocols = [
  { value: 'all', label: 'TCP / UDP' },
  { value: 'TCP', label: 'TCP' },
  { value: 'UDP', label: 'UDP' },
]
const searchModes = [
  { value: 'port', label: '端口号' },
  { value: 'process', label: '进程名' },
  { value: 'pid', label: 'PID' },
]

async function reveal(entry: PortEntry) {
  if (!entry.executablePath) return
  try {
    await revealItemInDir(entry.executablePath)
  } catch (cause) {
    ui.toast(`打开程序所在目录失败：${String(cause)}`)
  }
}
</script>

<template>
  <section class="flex h-full min-h-0 flex-col overflow-hidden bg-surface dark:bg-surface-dark">
    <div class="shrink-0 space-y-md border-b border-border p-lg dark:border-border-dark">
      <div class="flex items-baseline gap-sm">
        <h1 class="text-h1 text-primary dark:text-primary-dark">端口占用查询</h1>
        <span class="text-body-sm text-secondary dark:text-secondary-dark"
          >Windows · IPv4 / IPv6</span
        >
      </div>
      <form class="flex flex-wrap items-center gap-sm" @submit.prevent="query">
        <UiSelect v-model="filter.view" :options="views" aria-label="端口视图" class="w-36" />
        <UiSelect
          v-model="filter.protocol"
          :options="protocols"
          aria-label="协议筛选"
          class="w-32"
        />
        <UiSelect v-model="filter.by" :options="searchModes" aria-label="查询字段" class="w-28" />
        <UiInput
          v-model="filter.search"
          aria-label="查询条件"
          :placeholder="
            filter.by === 'port'
              ? '精确匹配本地端口，如 8080'
              : filter.by === 'pid'
                ? '输入完整 PID'
                : '输入进程名，如 node.exe'
          "
          :invalid="!!validation"
          class="min-w-48 flex-1"
        />
        <UiButton
          type="submit"
          variant="primary"
          class="w-24 shrink-0 justify-center"
          :loading="busy"
          :disabled="!available || !!closeTarget"
          >刷新</UiButton
        >
        <UiSwitch v-model="auto" label="自动刷新 · 3 秒" :disabled="!available" size="sm" />
      </form>
      <p v-if="auto && !visible" class="text-body-sm text-secondary dark:text-secondary-dark">
        页面在后台，自动刷新已暂停。
      </p>
      <UiAlert v-if="validation" tone="warning">{{ validation }}</UiAlert>
      <UiAlert v-if="error" tone="danger" title="查询未完成"
        >{{ error }}<span v-if="snapshot">；下方保留上次快照，请刷新后再关闭进程。</span></UiAlert
      >
      <UiAlert v-if="snapshot?.warnings.length" tone="warning" title="部分结果未能读取">{{
        snapshot.warnings.join('；')
      }}</UiAlert>
      <UiAlert v-if="notice" tone="success">{{ notice }}</UiAlert>
    </div>

    <div class="flex min-h-0 flex-1 flex-col p-lg">
      <UiEmptyState
        v-if="!desktop"
        title="请在 CoveKit 桌面应用中使用"
        description="浏览器预览无法访问本机端口信息。"
      />
      <UiEmptyState
        v-else-if="supported === false"
        title="目前仅支持 Windows"
        description="当前平台暂不支持端口占用查询。"
      />
      <UiEmptyState
        v-else-if="supported === null"
        :title="checking ? '正在检查平台支持…' : '无法确认平台支持状态'"
      >
        <UiButton v-if="!checking" @click="initialize">重试</UiButton>
      </UiEmptyState>
      <div v-else class="flex min-h-0 flex-1 flex-col" :aria-busy="busy" aria-live="polite">
        <div
          v-if="snapshot"
          class="mb-md flex shrink-0 flex-wrap items-baseline justify-between gap-sm"
        >
          <h2 class="text-h2 text-primary dark:text-primary-dark">
            {{ filter.view === 'listeners' ? '监听与绑定' : '全部连接' }} · {{ filtered.length }} 条
          </h2>
          <span class="text-body-sm text-secondary dark:text-secondary-dark">{{
            busy ? '正在刷新…' : `查询时间 ${queriedAt}`
          }}</span>
        </div>
        <UiEmptyState v-if="!snapshot && busy" title="正在读取本机端口…" />
        <UiEmptyState
          v-else-if="snapshot && !filtered.length && !validation"
          title="未发现匹配的端口记录"
          description="请核对查询条件，或切换全部连接。未发现记录不代表端口一定可以绑定。"
        />
        <UiScrollArea
          v-else-if="pageEntries.length"
          :key="page"
          axis="both"
          class="min-h-0 flex-1 rounded-lg border border-border dark:border-border-dark"
        >
          <UiTable
            :framed="false"
            :table-class="
              filter.view === 'all' ? 'table-fixed min-w-[1200px]' : 'table-fixed min-w-[1000px]'
            "
          >
            <colgroup>
              <col class="w-[88px]" />
              <col class="w-[184px]" />
              <col v-if="filter.view === 'all'" class="w-[200px]" />
              <col class="w-[112px]" />
              <col class="w-[144px]" />
              <col />
              <col class="w-[288px]" />
            </colgroup>
            <thead class="sticky top-0 z-10">
              <tr>
                <UiTableCell as="th" :resizable="false">协议</UiTableCell>
                <UiTableCell as="th" :resizable="false">本地地址 / 端口</UiTableCell>
                <UiTableCell v-if="filter.view === 'all'" as="th" :resizable="false"
                  >远端地址 / 端口</UiTableCell
                >
                <UiTableCell as="th" :resizable="false">状态</UiTableCell>
                <UiTableCell as="th" :resizable="false">进程 / PID</UiTableCell>
                <UiTableCell as="th" :resizable="false">程序路径 / 提示</UiTableCell>
                <UiTableCell as="th" :resizable="false" align="right">操作</UiTableCell>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(entry, index) in pageEntries" :key="`${entryKey(entry)}:${index}`">
                <UiTableCell content="technical" class="align-top"
                  ><p>{{ entry.protocol }}</p>
                  <p class="mt-xs">{{ entry.family }}</p></UiTableCell
                >
                <UiTableCell content="technical" class="align-top"
                  ><UiTooltip :content="addressText(entry.localAddress, entry.localPort)"
                    ><p class="truncate">
                      {{ addressText(entry.localAddress, entry.localPort) }}
                    </p></UiTooltip
                  ></UiTableCell
                >
                <UiTableCell v-if="filter.view === 'all'" content="technical" class="align-top"
                  ><UiTooltip
                    :content="
                      entry.remoteAddress === null
                        ? ''
                        : addressText(entry.remoteAddress, entry.remotePort ?? 0)
                    "
                    ><p class="truncate">
                      {{
                        entry.remoteAddress === null
                          ? '—'
                          : addressText(entry.remoteAddress, entry.remotePort ?? 0)
                      }}
                    </p></UiTooltip
                  ></UiTableCell
                >
                <UiTableCell content="technical" class="align-top"
                  ><UiTooltip :content="entry.state ?? '已绑定'"
                    ><p class="truncate">
                      {{ entry.state ?? '已绑定' }}
                    </p></UiTooltip
                  ></UiTableCell
                >
                <UiTableCell content="technical" class="align-top"
                  ><UiTooltip :content="entry.processName ?? ''"
                    ><p class="truncate">
                      {{ entry.processName ?? (entry.pid === 0 ? '无所属进程' : '进程名称未读取') }}
                    </p></UiTooltip
                  >
                  <p class="mt-xs whitespace-nowrap">PID {{ entry.pid }}</p></UiTableCell
                >
                <UiTableCell content="technical" class="align-top">
                  <UiTooltip v-if="entry.executablePath" :content="entry.executablePath"
                    ><p class="truncate">
                      {{ entry.executablePath }}
                    </p></UiTooltip
                  >
                  <UiTooltip v-if="entry.detailError" :content="entry.detailError"
                    ><p class="truncate text-warning-strong dark:text-warning-dark">
                      {{ entry.detailError }}
                    </p></UiTooltip
                  >
                  <p v-if="entry.pid === 0">此连接无可操作的进程</p>
                </UiTableCell>
                <UiTableCell content="action" align="right" class="align-top"
                  ><div class="flex justify-end gap-xs whitespace-nowrap">
                    <UiButton
                      size="sm"
                      variant="ghost"
                      @click="copyText(copyEntry(entry), '已复制端口与进程信息')"
                      >复制信息</UiButton
                    >
                    <UiButton
                      size="sm"
                      variant="ghost"
                      :disabled="!entry.executablePath"
                      @click="reveal(entry)"
                      >打开所在目录</UiButton
                    >
                    <UiButton
                      size="sm"
                      variant="ghost"
                      class="text-danger-strong dark:text-danger-dark"
                      :disabled="!entry.startedAt || entry.pid <= 4 || busy || !!error || closing"
                      @click="requestClose(entry)"
                      >关闭进程</UiButton
                    >
                  </div></UiTableCell
                >
              </tr>
            </tbody>
          </UiTable>
        </UiScrollArea>
        <div
          v-if="filtered.length > 100"
          class="mt-md flex shrink-0 flex-wrap items-center justify-between gap-sm"
        >
          <span class="text-body-sm text-secondary dark:text-secondary-dark">每页 100 条</span>
          <UiPagination v-model="page" :total-pages="totalPages" />
        </div>
        <p
          v-if="snapshot"
          class="mt-md shrink-0 text-body-sm text-secondary dark:text-secondary-dark"
        >
          TCP 显示连接状态，UDP 显示绑定端口。关闭进程会结束整个程序，操作后将重新查询。
        </p>
      </div>
    </div>
    <UiConfirmDialog
      :open="!!closeTarget"
      title="关闭端口使用进程"
      :message="`确定强制关闭 ${closeTarget?.processName || '所选进程'}（PID ${closeTarget?.pid ?? ''}）？该进程的所有窗口、连接和任务都会结束，未保存的内容可能丢失。`"
      confirm-label="确认关闭"
      danger
      :loading="closing"
      :error="closeError"
      @confirm="confirmClose"
      @close="cancelClose"
    />
  </section>
</template>
