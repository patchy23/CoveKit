<script setup lang="ts">
/** FileBrowser · SSH 文件页的路径工具栏、远程文件表格与键盘首字母定位。 */
import { nextTick, onMounted, ref, watch, type ComponentPublicInstance } from 'vue'
import type { RemoteFile } from './contracts'
import { formatBytes, formatTime } from './useSsh'
import { UiButton, UiTable, UiTableCell } from '@/core/ui'
import RemotePathToolbar from './RemotePathToolbar.vue'

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
      @upload="emit('upload')"
      @upload-directory="emit('uploadDirectory')"
      @download="emit('download')"
      @rename="emit('rename')"
      @delete="emit('delete')"
    />

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
            :class="
              selectedPath === file.path
                ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark'
                : 'hover:bg-border dark:hover:bg-border-dark'
            "
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
            <UiTableCell content="technical" class="px-[12px] py-[7px]">{{
              file.owner
            }}</UiTableCell>
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
  </div>
</template>
