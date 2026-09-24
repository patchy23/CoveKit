<script setup lang="ts">
/**
 * FileManagerPanes · 文件页双栏区（远程 FileBrowser + 中列传输按钮 + 本地 LocalBrowser + 拖拽遮罩）
 * 从 FileManagerTab 拆出（300 行红线）；纯布局装配，事件全部上抛。
 */
import { ref, nextTick, onMounted, onUnmounted } from 'vue'
import type { RemoteFile } from '../contracts'
import { UiIcon, UiIconButton, UiSplitPane } from '@/core/ui'
import { computed } from 'vue'
import { useFileDrag, type DragSide } from './useFileDrag'
import FileBrowser from './FileBrowser.vue'
import DirectoryBookmarks from './DirectoryBookmarks.vue'
import LocalBrowser from './LocalBrowser.vue'

const props = defineProps<{
  dragActive: boolean
  currentPath: string
  files: RemoteFile[]
  active?: boolean
  canGoBack: boolean
  sortKey: 'name' | 'modifiedAt'
  sortDirection: 'asc' | 'desc'
  remoteSelectedPaths: Set<string>
  remoteSelectedCount: number
  transferStatus?: string
  localInitialPath: string
  localSelectedPaths: Set<string>
  /** 书签按服务器隔离 */
  profileId?: string
  /** 全部传输项（状态栏传输按钮/面板） */
  bookmarksOpen: boolean
}>()

const emit = defineEmits<{
  (e: 'navigate', path: string): void
  (e: 'back'): void
  (e: 'up'): void
  (e: 'remoteRowClick', mouse: MouseEvent, file: RemoteFile): void
  (e: 'open', file: RemoteFile): void
  (e: 'remoteContext', mouse: MouseEvent, file: RemoteFile | null): void
  (e: 'sort', key: 'name' | 'modifiedAt'): void
  (e: 'uploadLocal'): void
  (e: 'localRowClick', mouse: MouseEvent, file: RemoteFile): void
  (e: 'localRowContext', mouse: MouseEvent, file: RemoteFile): void
  (e: 'localBlankContext', mouse: MouseEvent): void
  (e: 'localError', message: string): void
  (e: 'update:bookmarksOpen', value: boolean): void
  /** 远程栏拖到本地栏（下载） */
  (e: 'dropRemoteToLocal', items: RemoteFile[]): void
  /** 本地栏拖到远程栏（上传） */
  (e: 'dropLocalToRemote', items: RemoteFile[]): void
}>()

/** 两栏容器（拖拽目标判定用） */
const remotePane = ref<HTMLElement | null>(null)
const localPane = ref<HTMLElement | null>(null)
/** 本地浏览器实例（父级读取 files/currentPath/refresh 用，defineExpose 转发） */
const localBrowser = ref<InstanceType<typeof LocalBrowser> | null>(null)
/** 状态栏动作实例（父级「添加书签」菜单经此调用） */
const statusActions = ref<InstanceType<typeof DirectoryBookmarks> | null>(null)
const container = ref<HTMLElement>()
const width = ref(900),
  remoteWidth = ref(480)
const minRemote = computed(() => Math.min(260, width.value / 2))
const maxRemote = computed(() => Math.max(minRemote.value, width.value - 284))
let observer: ResizeObserver | undefined
onMounted(() => {
  observer = new ResizeObserver(() => {
    const next = container.value?.clientWidth
    if (next) width.value = next
  })
  if (container.value) observer.observe(container.value)
})
onUnmounted(() => observer?.disconnect())
const localOpen = ref(false)
const localVisited = ref(false)
function toggleLocal() {
  localVisited.value = true
  localOpen.value = !localOpen.value
}
async function locateLocal(path: string) {
  localVisited.value = true
  localOpen.value = true
  await nextTick()
  await localBrowser.value?.navigate(path)
}
defineExpose({ localBrowser, statusActions, locateLocal })

/* ── 双栏拖拽（pointer 自实现；拖拽后吞掉浏览器自动补发的 click，防误触选中） ── */
const { drag, dragTarget, onRowPointerDown } = useFileDrag({
  remotePane,
  localPane,
  onDrop: (source, _target, items) => {
    if (source === 'remote') emit('dropRemoteToLocal', items)
    else emit('dropLocalToRemote', items)
    window.addEventListener(
      'click',
      (e) => {
        e.stopPropagation()
        e.preventDefault()
      },
      { capture: true, once: true }
    )
  },
})

/** 行指针按下：转发到拖拽机（source 按栏定） */
function onRowPointerDownSide(event: PointerEvent, side: DragSide, file: RemoteFile) {
  onRowPointerDown(event, side, file)
}

/** 中列下载按钮：单项直下（父级走单选链路），多选取选中集走批量下载 */
function onMiddleDownload() {
  const picked = props.files.filter((f) => props.remoteSelectedPaths.has(f.path))
  emit('dropRemoteToLocal', picked)
}

/** 拖拽 ghost 文案（来源栏 + 数量） */
const dragLabel = computed(() => {
  const d = drag.value
  if (!d) return ''
  const n = d.items.length
  const action = d.source === 'local' ? '上传到远端' : '下载到本地'
  return n === 1 ? `⬆ ${d.items[0].name}（${action}）` : `⇅ ${n} 项（${action}）`
})
</script>

<template>
  <div
    v-if="dragActive"
    class="pointer-events-none absolute inset-[8px] z-[60] flex items-center justify-center rounded-lg border-2 border-dashed border-tertiary bg-tertiary-soft/90 text-h2 text-tertiary-strong shadow-card dark:border-tertiary-dark dark:bg-tertiary-soft-dark/90 dark:text-tertiary-dark"
  >
    释放文件，上传到 {{ currentPath }}
  </div>
  <div ref="container" class="flex min-h-0 flex-1">
    <UiSplitPane
      class="h-full min-w-0 flex-1"
      :model-value="localOpen ? Math.min(remoteWidth, maxRemote) : Math.max(0, width - 32)"
      :min="localOpen ? minRemote : 0"
      :max="localOpen ? maxRemote : width"
      :resizable="localOpen"
      label="调整远程与本地文件栏宽度"
      @update:model-value="remoteWidth = $event"
    >
      <template #primary>
        <div
          ref="remotePane"
          class="h-full min-w-0 overflow-hidden transition-shadow"
          :class="drag?.active && dragTarget === 'remote' ? 'shadow-[inset_0_0_0_2px_#F0562C]' : ''"
        >
          <FileBrowser
            class="h-full"
            :current-path="currentPath"
            :files="files"
            :active="active"
            :can-go-back="canGoBack"
            :sort-key="sortKey"
            :sort-direction="sortDirection"
            :selected-paths="remoteSelectedPaths"
            :selected-count="remoteSelectedCount"
            :transfer-status="transferStatus"
            @navigate="(p: string) => emit('navigate', p)"
            @back="emit('back')"
            @up="emit('up')"
            @row-click="(e: MouseEvent, f: RemoteFile) => emit('remoteRowClick', e, f)"
            @open="(f: RemoteFile) => emit('open', f)"
            @context="(e: MouseEvent, f: RemoteFile | null) => emit('remoteContext', e, f)"
            @sort="(k: 'name' | 'modifiedAt') => emit('sort', k)"
            @row-pointer-down="
              (e: PointerEvent, f: RemoteFile) => onRowPointerDownSide(e, 'remote', f)
            "
          >
            <template #file-actions><slot name="file-actions" /></template>
            <template #status-actions>
              <DirectoryBookmarks
                ref="statusActions"
                :profile-id="profileId"
                :path="currentPath"
                :open="bookmarksOpen"
                @update:open="emit('update:bookmarksOpen', $event)"
                @navigate="(p: string) => emit('navigate', p)"
              />
              <slot name="transfers" />
            </template>
          </FileBrowser>
        </div>
      </template>
      <template #secondary
        ><div class="flex h-full min-w-0">
          <!-- 中列传输按钮：箭头方向与栏位几何一致（服务器在左、本地在右） -->
          <div
            v-show="localOpen"
            class="flex w-[42px] shrink-0 flex-col items-center justify-center gap-[8px] border-l border-border dark:border-border-dark"
          >
            <UiIconButton
              label="上传到远端"
              title="上传到远端：把本地选中的文件/目录传到左侧远端当前目录"
              @click="emit('uploadLocal')"
            >
              <UiIcon name="arrow-left" :size="16" />
            </UiIconButton>
            <UiIconButton
              label="下载到右侧目录"
              title="下载到右侧目录：把远端选中的文件/目录传到右侧本地当前目录"
              @click="onMiddleDownload"
            >
              <UiIcon name="arrow-right" :size="16" />
            </UiIconButton>
          </div>

          <div
            v-if="!localOpen"
            class="flex w-[28px] shrink-0 flex-col items-center gap-sm border-l border-border py-sm dark:border-border-dark"
          >
            <UiIconButton size="xs" label="展开本地文件" @click="toggleLocal"
              ><UiIcon name="chevrons-left" :size="14"
            /></UiIconButton>
            <span
              class="text-caption text-secondary [writing-mode:vertical-rl] dark:text-secondary-dark"
              >本地文件</span
            >
          </div>
          <!-- 右栏：本地目录 -->
          <div
            v-show="localOpen"
            ref="localPane"
            class="min-w-0 flex-1 overflow-hidden transition-shadow"
            :class="
              drag?.active && dragTarget === 'local' ? 'shadow-[inset_0_0_0_2px_#F0562C]' : ''
            "
          >
            <LocalBrowser
              v-if="localVisited"
              ref="localBrowser"
              class="h-full"
              :initial-path="localInitialPath"
              :selected-paths="localSelectedPaths"
              @collapse="toggleLocal"
              @row-click="(e: MouseEvent, f: RemoteFile) => emit('localRowClick', e, f)"
              @row-context="(e: MouseEvent, f: RemoteFile) => emit('localRowContext', e, f)"
              @blank-context="(e: MouseEvent) => emit('localBlankContext', e)"
              @error="(m: string) => emit('localError', m)"
              @row-pointer-down="
                (e: PointerEvent, f: RemoteFile) => onRowPointerDownSide(e, 'local', f)
              "
            />
          </div></div
      ></template>
    </UiSplitPane>
  </div>

  <!-- 拖拽 ghost（跟随指针；越阈值后显示，指向对侧栏时高亮） -->
  <div
    v-if="drag?.active"
    class="pointer-events-none fixed z-[70] whitespace-nowrap rounded-md border border-tertiary bg-surface px-[8px] py-[4px] text-body-sm text-tertiary-strong shadow-card dark:border-tertiary-dark dark:bg-surface-dark dark:text-tertiary-dark"
    :style="{ left: `${drag.x + 12}px`, top: `${drag.y + 12}px` }"
  >
    {{ dragLabel }}
  </div>
</template>
