<script setup lang="ts">
/** FileBrowser · SSH 文件页的路径工具栏、远程文件表格与键盘首字母定位。 */
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  type ComponentPublicInstance,
} from 'vue'
import type { RemoteFile } from './contracts'
import { formatBytes, formatTime } from './useSsh'
import { UiButton, UiInput, UiTable, UiTableCell } from '@/core/ui'

const props = defineProps<{
  currentPath: string
  files: RemoteFile[]
  active?: boolean
  canGoBack?: boolean
  selectedPath?: string
  selectedName?: string
  transferStatus?: string
  sortKey: 'name' | 'modifiedAt'
  sortDirection: 'asc' | 'desc'
}>()

const emit = defineEmits<{
  (event: 'navigate', path: string): void
  (event: 'back'): void
  (event: 'up'): void
  (event: 'upload'): void
  (event: 'uploadDirectory'): void
  (event: 'download'): void
  (event: 'rename'): void
  (event: 'delete'): void
  (event: 'select', file: RemoteFile): void
  (event: 'open', file: RemoteFile): void
  (event: 'context', mouse: MouseEvent, file: RemoteFile | null): void
  (event: 'sort', key: 'name' | 'modifiedAt'): void
}>()

const listViewport = ref<HTMLElement | null>(null)
const rowElements = new Map<string, HTMLElement>()
const editingPath = ref(false)
const pathDraft = ref(props.currentPath)
const pathInput = ref<{ focus: () => void; select: () => void } | null>(null)
const pathViewport = ref<HTMLElement | null>(null)
const pathContent = ref<HTMLElement | null>(null)
const pathOverflowing = ref(false)
let pathResizeObserver: ResizeObserver | null = null

const pathSegments = computed(() => {
  const parts = props.currentPath.split('/').filter(Boolean)
  return [
    { label: '/', path: '/' },
    ...parts.map((part, index) => ({
      label: part,
      path: `/${parts.slice(0, index + 1).join('/')}`,
    })),
  ]
})

function beginPathEdit() {
  pathDraft.value = props.currentPath
  editingPath.value = true
  nextTick(() => {
    pathInput.value?.focus()
    pathInput.value?.select()
  })
}

function updatePathOverflow() {
  const viewport = pathViewport.value
  const content = pathContent.value
  pathOverflowing.value = Boolean(
    viewport && content && content.scrollWidth > viewport.clientWidth + 1
  )
}

function cancelPathEdit() {
  pathDraft.value = props.currentPath
  editingPath.value = false
  nextTick(updatePathOverflow)
}

function submitPath() {
  const value = pathDraft.value.trim()
  if (!value) return
  const normalized = value.startsWith('/') ? value : `/${value}`
  editingPath.value = false
  emit('navigate', normalized)
}

function navigateSegment(path: string) {
  editingPath.value = false
  emit('navigate', path)
}

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
  const currentIndex = matches.findIndex((file) => file.path === props.selectedPath)
  const target = matches[currentIndex >= 0 ? (currentIndex + 1) % matches.length : 0]
  emit('select', target)
  await nextTick()
  const row = rowElements.get(target.path)
  const viewport = listViewport.value
  if (row && viewport) {
    const headerHeight = viewport.querySelector('thead')?.getBoundingClientRect().height ?? 0
    viewport.scrollTo({ top: Math.max(0, row.offsetTop - headerHeight), behavior: 'smooth' })
  }
}

function selectFile(file: RemoteFile) {
  emit('select', file)
  nextTick(focusList)
}

onMounted(() => {
  if (props.active !== false) nextTick(focusList)
  pathResizeObserver = new ResizeObserver(updatePathOverflow)
  if (pathViewport.value) pathResizeObserver.observe(pathViewport.value)
  nextTick(updatePathOverflow)
})

onBeforeUnmount(() => pathResizeObserver?.disconnect())

watch(pathViewport, (element, previous) => {
  if (previous) pathResizeObserver?.unobserve(previous)
  if (element) pathResizeObserver?.observe(element)
  nextTick(updatePathOverflow)
})

watch(
  () => props.active,
  (active) => {
    if (active) nextTick(focusList)
  }
)

watch(
  () => props.currentPath,
  (path) => {
    pathDraft.value = path
    editingPath.value = false
    nextTick(updatePathOverflow)
  }
)
</script>

<template>
  <div
    class="flex shrink-0 items-center gap-[8px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
  >
    <UiButton
      variant="ghost"
      size="sm"
      title="返回上一次目录"
      :disabled="!canGoBack"
      @click="emit('back')"
    >
      ← 后退
    </UiButton>
    <UiButton variant="ghost" size="sm" title="上级目录" @click="emit('up')"> ↑ 上级 </UiButton>
    <UiInput
      v-if="editingPath"
      ref="pathInput"
      v-model="pathDraft"
      size="sm"
      class="flex-1 font-mono"
      spellcheck="false"
      aria-label="输入远程目录路径"
      @blur="cancelPathEdit"
      @keyup.enter="submitPath"
      @keyup.esc="cancelPathEdit"
    />
    <div
      v-else
      class="flex h-[30px] min-w-0 flex-1 cursor-text items-center overflow-hidden rounded-md border border-border-strong bg-surface-muted pl-[4px] dark:border-border-strong-dark dark:bg-surface-muted-dark"
      title="点击空白处输入完整路径"
      @click="beginPathEdit"
    >
      <div
        ref="pathViewport"
        class="relative flex min-w-0 flex-1 overflow-hidden"
        :class="pathOverflowing ? 'justify-end' : 'justify-start'"
      >
        <span
          v-if="pathOverflowing"
          class="absolute inset-y-0 left-0 z-10 flex items-center bg-surface-muted px-[6px] text-body-sm text-text-muted dark:bg-surface-muted-dark dark:text-text-muted-dark"
          aria-hidden="true"
        >
          …
        </span>
        <nav
          ref="pathContent"
          class="flex min-w-max shrink-0 items-center"
          aria-label="远程目录路径"
        >
          <template v-for="(segment, index) in pathSegments" :key="segment.path">
            <span
              v-if="index > 0"
              class="px-[1px] text-caption text-text-muted dark:text-text-muted-dark"
              aria-hidden="true"
            >
              ›
            </span>
            <UiButton
              variant="ghost"
              size="xs"
              class="font-mono"
              :title="`进入 ${segment.path}`"
              @click.stop="navigateSegment(segment.path)"
            >
              {{ segment.label }}
            </UiButton>
          </template>
        </nav>
      </div>
      <span class="h-full w-[36px] shrink-0" aria-hidden="true" />
    </div>
    <UiButton size="sm" @click="emit('upload')">上传文件</UiButton>
    <UiButton size="sm" @click="emit('uploadDirectory')">上传目录</UiButton>
    <UiButton size="sm" @click="emit('download')">下载</UiButton>
    <UiButton size="sm" @click="emit('rename')">重命名</UiButton>
    <UiButton size="sm" class="text-danger-strong dark:text-danger-dark" @click="emit('delete')">
      删除
    </UiButton>
  </div>

  <div
    ref="listViewport"
    class="min-h-0 flex-1 overflow-y-auto outline-none"
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
          <UiTableCell as="th" class="px-[12px] py-[8px]">
            <UiButton variant="ghost" size="xs" @click="emit('sort', 'name')">
              名称 {{ sortKey === 'name' ? (sortDirection === 'asc' ? '↑' : '↓') : '' }}
            </UiButton>
          </UiTableCell>
          <UiTableCell as="th" class="w-[100px] px-[12px] py-[8px]">大小</UiTableCell>
          <UiTableCell as="th" class="w-[132px] px-[12px] py-[8px]">
            <UiButton variant="ghost" size="xs" @click="emit('sort', 'modifiedAt')">
              修改时间
              {{ sortKey === 'modifiedAt' ? (sortDirection === 'asc' ? '↑' : '↓') : '' }}
            </UiButton>
          </UiTableCell>
          <UiTableCell as="th" class="w-[110px] px-[12px] py-[8px]">权限</UiTableCell>
          <UiTableCell as="th" class="w-[80px] px-[12px] py-[8px]">所有者</UiTableCell>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="file in files"
          :key="file.path"
          :ref="(element) => setRowElement(file.path, element)"
          class="cursor-pointer border-b border-border/50 transition-colors dark:border-border-dark/50"
          :data-selected="selectedPath === file.path"
          @click="selectFile(file)"
          @dblclick="emit('open', file)"
          @contextmenu.stop="emit('context', $event, file)"
        >
          <UiTableCell content="technical" class="px-[12px] py-[7px]">
            <span class="mr-[6px]">{{ file.isDir ? '📁' : '📄' }}</span>
            <span :class="{ 'font-medium': file.isDir }">{{ file.name }}</span>
          </UiTableCell>
          <UiTableCell content="numeric" class="px-[12px] py-[7px]">
            {{ file.isDir ? '-' : formatBytes(file.size) }}
          </UiTableCell>
          <UiTableCell content="numeric" class="px-[12px] py-[7px]">{{
            formatTime(file.modifiedAt)
          }}</UiTableCell>
          <UiTableCell content="technical" class="px-[12px] py-[7px]">{{
            file.permissions
          }}</UiTableCell>
          <UiTableCell content="technical" class="px-[12px] py-[7px]">{{ file.owner }}</UiTableCell>
        </tr>
      </tbody>
    </UiTable>
  </div>

  <div
    class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
  >
    <span>{{ currentPath }}</span>
    <span>{{ files.length }} 个项目</span>
    <span v-if="transferStatus" class="text-tertiary-strong dark:text-tertiary-dark">
      {{ transferStatus }}
    </span>
    <span v-if="selectedName" class="text-tertiary-strong dark:text-tertiary-dark">
      已选：{{ selectedName }}
    </span>
  </div>
</template>
