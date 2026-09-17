<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/**
 * FileStatusActions · 远程栏状态栏右侧双按钮（☆书签下拉 + ⇅传输进度按钮/上拉明细面板）
 * 关闭规则：书签下拉点击外部/选书签后关闭；传输面板只能 ✕ 或再点进度条按钮关闭（禁 Esc/遮罩）。
 */
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import type { TransferItem } from './useFileTransfer'
import { useBookmarks } from './useBookmarks'
import { UiButton, UiIcon, UiIconButton } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'

const props = defineProps<{
  /** 所属服务器（书签按 profile 隔离） */
  profileId?: string
  /** 全部传输项（面板明细） */
  transfers: TransferItem[]
}>()

const emit = defineEmits<{
  /** 书签跳转 */
  (e: 'navigate', path: string): void
  (e: 'cancel', id: string): void
  (e: 'cancelAll'): void
}>()

/* ── 书签 ── */
const {
  bookmarks,
  open: bookmarkOpen,
  add,
  remove,
  go,
} = useBookmarks({
  profileId: () => props.profileId,
  navigate: (path) => emit('navigate', path),
})

/** 「添加书签」菜单入口（经 Panes 转发调用） */
defineExpose({ addBookmark: add })

/** 书签下拉点击外部关闭 */
const bookmarkRoot = ref<HTMLElement | null>(null)
function onDocPointerDown(e: PointerEvent) {
  if (bookmarkOpen.value && !bookmarkRoot.value?.contains(e.target as Node)) {
    bookmarkOpen.value = false
  }
}
onMounted(() => document.addEventListener('pointerdown', onDocPointerDown))
onBeforeUnmount(() => document.removeEventListener('pointerdown', onDocPointerDown))

/* ── 传输 ── */
const panelOpen = ref(false)
const cancelAllConfirm = ref(false)

const activeTransfers = computed(() => props.transfers.filter((t) => !t.done || t.error))
const runningTransfers = computed(() => props.transfers.filter((t) => !t.done))

/** 全部取消确认通过（多语句内联处理器会被 vite 拒绝，必须抽函数） */
function onCancelAllConfirmed() {
  emit('cancelAll')
  cancelAllConfirm.value = false
}

/** 总进度 = 已传总和 / 总量总和（无总量信息时按完成项比例） */
const totalProgress = computed(() => {
  const list = activeTransfers.value
  if (!list.length) return 0
  const withTotal = list.filter((t) => t.total > 0)
  if (withTotal.length) {
    const transferred = withTotal.reduce((s, t) => s + t.transferred, 0)
    const total = withTotal.reduce((s, t) => s + t.total, 0)
    return total > 0 ? Math.min(100, Math.round((transferred / total) * 100)) : 0
  }
  const done = list.filter((t) => t.done).length
  return Math.round((done / list.length) * 100)
})

/** 当前任务名：固定 12 字符宽度，超出省略 */
const currentTaskLabel = computed(() => {
  const current = runningTransfers.value[0]
  if (!current) return ''
  const name = current.label
  return name.length > 12 ? `${name.slice(0, 12)}…` : name
})
</script>

<template>
  <div class="flex shrink-0 items-center gap-[2px]">
    <!-- 书签按钮 + 下拉 -->
    <div ref="bookmarkRoot" class="relative">
      <UiIconButton
        label="书签"
        size="sm"
        title="目录书签"
        :class="bookmarkOpen ? 'text-tertiary-strong dark:text-tertiary-dark' : ''"
        @click="bookmarkOpen = !bookmarkOpen"
      >
        <UiIcon name="star" :size="14" />
      </UiIconButton>
      <div
        v-if="bookmarkOpen"
        class="absolute bottom-full right-0 z-[220] mb-[4px] max-h-[240px] w-[240px] overflow-y-auto rounded-lg border border-border bg-surface py-[4px] shadow-card dark:border-border-dark dark:bg-surface-dark"
      >
        <div
          v-if="!bookmarks.length"
          class="px-[12px] py-[10px] text-caption text-text-muted dark:text-text-muted-dark"
        >
          暂无书签（右键目录可添加）
        </div>
        <div
          v-for="bm in bookmarks"
          :key="bm.id"
          class="group flex cursor-pointer items-center gap-[6px] px-[10px] py-[6px] hover:bg-border dark:hover:bg-border-dark"
          @click="go(bm.path)"
        >
          <span class="shrink-0 text-body-sm">⭐</span>
          <span class="min-w-0 flex-1">
            <span class="block truncate text-body-sm">{{ bm.name }}</span>
            <span class="block truncate font-mono text-caption text-text-muted">{{ bm.path }}</span>
          </span>
          <UiButton
            variant="ghost"
            size="xs"
            class="!h-auto shrink-0 !px-[4px] !py-[1px] text-text-muted opacity-0 transition-opacity hover:text-danger-strong group-hover:opacity-100"
            title="删除书签"
            @click.stop="remove(bm.id)"
          >
            ✕
          </UiButton>
        </div>
      </div>
    </div>

    <!-- 传输按钮（本体即进度条）+ 上拉面板 -->
    <div class="relative">
      <!-- 传输按钮本体即进度条；用 div[role=button] 规避原生 button（公共组件契约限制自定义布局） -->
      <UiTooltip :content="panelOpen ? '关闭传输明细' : '传输明细'">
        <div
          role="button"
          tabindex="0"
          class="relative h-[24px] min-w-[64px] cursor-pointer select-none overflow-hidden rounded-md border border-border px-[8px] text-caption transition-colors dark:border-border-dark"
          :class="
            runningTransfers.length
              ? 'text-secondary dark:text-secondary-dark'
              : 'text-text-muted dark:text-text-muted-dark'
          "
          @click="panelOpen = !panelOpen"
          @keydown.enter="panelOpen = !panelOpen"
        >
          <!-- 进度填充动画 -->
          <span
            v-if="runningTransfers.length"
            class="absolute inset-y-0 left-0 bg-tertiary-soft transition-[width] duration-300 dark:bg-tertiary-soft-dark"
            :style="{ width: `${totalProgress}%` }"
          ></span>
          <span class="relative">
            <template v-if="runningTransfers.length">
              ⇅ {{ runningTransfers.length }} 项 · {{ currentTaskLabel }}
            </template>
            <template v-else>⇅ 传输</template>
          </span>
        </div>
      </UiTooltip>

      <!-- 上拉明细面板（只能 ✕ 或再点进度条按钮关闭；禁 Esc/遮罩） -->
      <div
        v-if="panelOpen"
        class="absolute bottom-full right-0 z-[220] mb-[4px] flex max-h-[320px] w-[380px] flex-col rounded-lg border border-border bg-surface shadow-card dark:border-border-dark dark:bg-surface-dark"
      >
        <div
          class="flex shrink-0 items-center justify-between border-b border-border px-[12px] py-[8px] dark:border-border-dark"
        >
          <span class="text-body-sm font-medium">传输明细</span>
          <div class="flex items-center gap-[6px]">
            <UiButton
              v-if="runningTransfers.length"
              variant="ghost"
              size="xs"
              class="!h-auto !px-[4px] !py-[1px] text-caption text-danger-strong dark:text-danger-dark"
              @click="cancelAllConfirm = true"
            >
              全部取消
            </UiButton>
            <UiButton
              variant="ghost"
              size="xs"
              class="!h-auto !px-[4px] !py-[1px] text-text-muted"
              title="关闭"
              @click="panelOpen = false"
            >
              ✕
            </UiButton>
          </div>
        </div>
        <div class="min-h-0 flex-1 overflow-y-auto px-[12px] py-[6px]">
          <div
            v-if="!activeTransfers.length"
            class="py-[14px] text-center text-caption text-text-muted dark:text-text-muted-dark"
          >
            暂无传输任务
          </div>
          <div
            v-for="item in activeTransfers"
            :key="item.id"
            class="flex items-center gap-[8px] py-[5px] text-caption"
          >
            <span
              class="shrink-0 rounded-full bg-neutral px-[7px] py-[1px] font-medium text-text-muted dark:bg-neutral-dark dark:text-text-muted-dark"
            >
              {{ item.kind === 'upload' ? '上传' : '下载' }}
            </span>
            <span class="min-w-0 flex-1">
              <span class="block truncate text-secondary dark:text-secondary-dark">{{
                item.label
              }}</span>
              <!-- 单项进度条动画 -->
              <span
                v-if="!item.done && item.total > 0"
                class="mt-[2px] block h-[3px] overflow-hidden rounded-full bg-border dark:bg-border-dark"
              >
                <span
                  class="block h-full bg-tertiary transition-[width] duration-300"
                  :style="{
                    width: `${Math.min(100, Math.round((item.transferred / item.total) * 100))}%`,
                  }"
                ></span>
              </span>
            </span>
            <span v-if="item.total > 0" class="shrink-0 font-mono text-text-muted">
              {{ Math.min(100, Math.round((item.transferred / item.total) * 100)) }}%
            </span>
            <span v-if="item.error" class="shrink-0 text-danger-strong dark:text-danger-dark">{{
              item.error
            }}</span>
            <UiButton
              v-if="!item.done"
              variant="ghost"
              size="xs"
              class="!h-auto shrink-0 !px-[4px] !py-[1px] text-caption text-danger-strong dark:text-danger-dark"
              @click="emit('cancel', item.id)"
            >
              取消
            </UiButton>
          </div>
        </div>
      </div>
    </div>

    <ConfirmDialog
      :open="cancelAllConfirm"
      title="取消全部传输"
      :message="`将取消进行中的 ${runningTransfers.length} 个传输任务。`"
      confirm-label="全部取消"
      danger
      @close="cancelAllConfirm = false"
      @confirm="onCancelAllConfirmed"
    />
  </div>
</template>
