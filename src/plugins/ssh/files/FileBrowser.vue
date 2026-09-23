<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { UiTooltip } from '@/core/ui'
/** FileBrowser · SSH 文件页的路径工具栏、远程文件表格与键盘首字母定位。 */
import { nextTick, onMounted, ref, watch, type ComponentPublicInstance } from 'vue'
import type { RemoteFile } from '../contracts'
import { formatBytes, formatTime } from '../connection/useSsh'
import { UiButton, UiStatusBar, UiTable, UiTableCell } from '@/core/ui'
import RemotePathToolbar from './RemotePathToolbar.vue'

const props = defineProps<{
  currentPath: string
  files: RemoteFile[]
  active?: boolean
  canGoBack?: boolean
  /** 多选集合（path 集合；空集 = 未选） */
  selectedPaths?: Set<string>
  /** 已选数量（状态栏展示） */
  selectedCount?: number
  transferStatus?: string
  sortKey: 'name' | 'modifiedAt'
  sortDirection: 'asc' | 'desc'
}>()

const emit = defineEmits<{
  (event: 'navigate', path: string): void
  (event: 'back'): void
  (event: 'up'): void
  (event: 'rowClick', mouse: MouseEvent, file: RemoteFile): void
  (event: 'open', file: RemoteFile): void
  (event: 'context', mouse: MouseEvent, file: RemoteFile | null): void
  /** 行指针按下（双栏拖拽起点） */
  (event: 'rowPointerDown', mouse: PointerEvent, file: RemoteFile): void
  (event: 'sort', key: 'name' | 'modifiedAt'): void
}>()

const listViewport = ref<HTMLElement | null>(null)
const rowElements = new Map<string, HTMLElement>()

function setRowElement(path: string, element: Element | ComponentPublicInstance | null) {
  if (element instanceof HTMLElement) rowElements.set(path, element)
  else rowElements.delete(path)
}

function focusList() {
  listViewport.value?.focus({ preventScroll: true })
}

/** 单字符循环匹配文件名前缀；命中行进入选中态并滚动到列表顶部。 */
async function jumpByInitial(event: KeyboardEvent) {
  if (
    event.isComposing ||
    event.repeat ||
    event.ctrlKey ||
    event.altKey ||
    event.metaKey ||
    event.key.length !== 1
  ) {
    return
  }
  const initial = event.key.normalize('NFKC').toLocaleLowerCase()
  const matches = props.files.filter((file) =>
    file.name.normalize('NFKC').toLocaleLowerCase().startsWith(initial)
  )
  if (!matches.length) return
  event.preventDefault()
  // 循环定位：从「当前已选项中最后一个匹配」的下一个开始
  const currentIndex = matches.findIndex((file) => props.selectedPaths?.has(file.path))
  const target = matches[currentIndex >= 0 ? (currentIndex + 1) % matches.length : 0]
  emit('rowClick', new MouseEvent('click'), target)
  await nextTick()
  const row = rowElements.get(target.path)
  const viewport = listViewport.value
  if (row && viewport) {
    const headerHeight = viewport.querySelector('thead')?.getBoundingClientRect().height ?? 0
    viewport.scrollTo({ top: Math.max(0, row.offsetTop - headerHeight), behavior: 'smooth' })
  }
}

function selectFile(event: MouseEvent, file: RemoteFile) {
  emit('rowClick', event, file)
  nextTick(focusList)
}

onMounted(() => {
  if (props.active !== false) nextTick(focusList)
})

watch(
  () => props.active,
  (active) => {
    if (active) nextTick(focusList)
  }
)
</script>

<template>
  <!-- 必须单根：多根片段会让父组件的 class 透传被 Vue 静默丢弃（flex 尺寸丢失 → 列表塌掉显示不全） -->
  <div class="flex h-full min-h-0 flex-col">
    <RemotePathToolbar
      :current-path="currentPath"
      :can-go-back="canGoBack"
      @navigate="emit('navigate', $event)"
      @back="emit('back')"
      @up="emit('up')"
      ><slot name="file-actions"
    /></RemotePathToolbar>

    <UiScrollArea as-child axis="both">
      <div
        ref="listViewport"
        class="min-h-0 flex-1 outline-none"
        tabindex="0"
        aria-label="远程文件列表，输入首字母可循环定位"
        @keydown="jumpByInitial"
        @contextmenu="emit('context', $event, null)"
      >
        <UiTable :framed="false" :styled="false" table-class="text-body-sm">
          <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
            <tr
              class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
            >
              <UiTableCell as="th" class="min-w-[160px] px-[12px] py-[8px]">
                <UiButton variant="ghost" size="xs" @click="emit('sort', 'name')">
                  名称 {{ sortKey === 'name' ? (sortDirection === 'asc' ? '↑' : '↓') : '' }}
                </UiButton>
              </UiTableCell>
              <UiTableCell as="th" class="w-[76px] px-[12px] py-[8px]">大小</UiTableCell>
              <UiTableCell as="th" class="w-[118px] px-[12px] py-[8px]">
                <UiButton variant="ghost" size="xs" @click="emit('sort', 'modifiedAt')">
                  修改时间
                  {{ sortKey === 'modifiedAt' ? (sortDirection === 'asc' ? '↑' : '↓') : '' }}
                </UiButton>
              </UiTableCell>
              <UiTableCell as="th" class="w-[92px] px-[12px] py-[8px]">权限</UiTableCell>
              <UiTableCell as="th" class="w-[88px] px-[12px] py-[8px]">所有者</UiTableCell>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="file in files"
              :key="file.path"
              :ref="(element) => setRowElement(file.path, element)"
              class="cursor-pointer border-b border-border/50 transition-colors dark:border-border-dark/50"
              :class="
                selectedPaths?.has(file.path)
                  ? 'bg-tertiary-soft shadow-[inset_4px_0_0_0_#F0562C] dark:bg-tertiary-soft-dark'
                  : 'hover:bg-border dark:hover:bg-border-dark'
              "
              @click="selectFile($event, file)"
              @dblclick="emit('open', file)"
              @contextmenu.stop="emit('context', $event, file)"
              @pointerdown="emit('rowPointerDown', $event, file)"
            >
              <UiTableCell content="technical" class="max-w-0 px-[12px] py-[7px]">
                <!-- 长文件名截断不换行：auto 布局下 max-w-0 单元格 + 内层 truncate（不挤掉其他列），完整名走 title -->
                <UiTooltip :content="file.name">
                  <span class="block truncate">
                    <span class="mr-[6px]">{{ file.isDir ? '📁' : '📄' }}</span>
                    <span :class="{ 'font-medium': file.isDir }">{{ file.name }}</span>
                  </span>
                </UiTooltip>
              </UiTableCell>
              <UiTableCell content="numeric" class="whitespace-nowrap px-[12px] py-[7px]">
                {{ file.isDir ? '-' : formatBytes(file.size) }}
              </UiTableCell>
              <UiTableCell content="numeric" class="whitespace-nowrap px-[12px] py-[7px]">{{
                formatTime(file.modifiedAt)
              }}</UiTableCell>
              <UiTableCell content="technical" class="whitespace-nowrap px-[12px] py-[7px]">{{
                file.permissions
              }}</UiTableCell>
              <UiTableCell content="technical" class="whitespace-nowrap px-[12px] py-[7px]">{{
                file.owner
              }}</UiTableCell>
            </tr>
          </tbody>
        </UiTable>
      </div>
    </UiScrollArea>

    <UiStatusBar size="md">
      <span class="min-w-0 flex-1 truncate select-text">{{ currentPath }}</span>
      <span>{{ files.length }} 个项目</span>
      <span v-if="transferStatus" class="text-tertiary-strong dark:text-tertiary-dark">
        {{ transferStatus }}
      </span>
      <span v-if="selectedCount" class="text-tertiary-strong dark:text-tertiary-dark">
        已选 {{ selectedCount }} 项
      </span>
      <template #trailing><slot name="status-actions" /></template>
    </UiStatusBar>
  </div>
</template>
