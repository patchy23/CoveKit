<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { UiTooltip } from '@/core/ui'
/**
 * LocalBrowser · 双栏文件管理的本地侧（目录浏览 + 选中）
 * ssh_local_list 读目录；双击进入目录；选中文件后由中间列按钮发起上传。
 */
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { RemoteFile } from '../contracts'
import { ipc } from '../ipc'
import { formatBytes, formatTime } from '../connection/useSsh'
import PathBreadcrumbs from './PathBreadcrumbs.vue'
import { UiIcon, UiIconButton, UiStatusBar, UiTable, UiTableCell, UiToolbar } from '@/core/ui'

const props = defineProps<{
  /** 初始目录（默认取设置里的默认下载目录） */
  initialPath: string
  /** 多选集合（path 集合） */
  selectedPaths?: Set<string>
}>()

const emit = defineEmits<{
  /** 行点击（携带鼠标事件，多选语义在父级 useFileSelection） */
  (e: 'rowClick', mouse: MouseEvent, file: RemoteFile): void
  /** 空白处右键（新建/刷新菜单） */
  (e: 'blankContext', mouse: MouseEvent): void
  (e: 'rowContext', mouse: MouseEvent, file: RemoteFile): void
  /** 行指针按下（双栏拖拽起点） */
  (e: 'rowPointerDown', mouse: PointerEvent, file: RemoteFile): void
  (e: 'error', message: string): void
  (e: 'collapse'): void
}>()

const currentPath = ref(props.initialPath)
const files = ref<RemoteFile[]>([])
const loading = ref(false)
const parentPath = ref<string | null>(null)
const atDrives = computed(() => currentPath.value === '')
let generation = 0
onUnmounted(() => generation++)
async function navigate(path: string) {
  const request = ++generation
  loading.value = true
  try {
    // 规范化（分隔符/盘符尾斜杠/虚拟根）全在后端：''、'/'、'\' 都会得到驱动器列表
    const result = await ipc.sshLocalList(path)
    if (request !== generation) return
    if (!result.ok) {
      emit('error', result.error ?? '读取目录失败')
      return
    }
    parentPath.value = result.parentPath ?? null
    currentPath.value = result.path
    files.value = result.files
  } catch (error) {
    if (request === generation) emit('error', String(error))
  } finally {
    if (request === generation) loading.value = false
  }
}

function goUp() {
  // '' 是合法的「上级」（驱动器视图），只能用 null 判断
  if (parentPath.value !== null) void navigate(parentPath.value)
}

function open(file: RemoteFile) {
  if (file.isDir) void navigate(file.path)
  return
}

function onRowClick(event: MouseEvent, file: RemoteFile) {
  emit('rowClick', event, file)
}

onMounted(async () => {
  const request = generation
  try {
    const [path, warning] = await ipc.sshLocalDefaultDirectory(props.initialPath)
    if (request !== generation) return
    if (warning) emit('error', warning)
    await navigate(path)
  } catch (error) {
    if (request === generation) emit('error', String(error))
  }
})

defineExpose({
  navigate,
  /** 当前目录 */
  currentPath,
  /** 当前目录文件列表（父级多选模型用） */
  files,
  /** 重新加载当前目录 */
  refresh: () => navigate(currentPath.value),
})
</script>

<template>
  <div class="flex min-h-0 flex-col border-l border-border dark:border-border-dark">
    <!-- 工具栏：上级 + 面包屑路径（与远程侧同款）+ 刷新 -->
    <UiToolbar bordered>
      <UiIconButton size="xs" label="收起本地文件" @click="emit('collapse')"
        ><UiIcon name="chevron-right" :size="14"
      /></UiIconButton>
      <!-- 盘符根的上级是 ''（驱动器视图），禁用判定必须用 null 比较（'' 是假值会误禁用） -->
      <UiIconButton
        label="上级"
        size="sm"
        title="上级目录"
        :disabled="parentPath === null"
        @click="goUp"
      >
        <UiIcon name="arrow-up" :size="14" />
      </UiIconButton>
      <PathBreadcrumbs
        v-if="!atDrives"
        class="min-w-0 flex-1"
        :path="currentPath"
        :separator="'\\'"
        @navigate="navigate"
      />
      <span
        v-else
        class="min-w-0 flex-1 px-[8px] text-body-sm text-secondary dark:text-secondary-dark"
        >此电脑</span
      >
      <UiIconButton label="刷新" size="sm" title="刷新" @click="navigate(currentPath)">
        <UiIcon name="refresh" :size="14" />
      </UiIconButton>
    </UiToolbar>

    <!-- 列表（与远程侧同款 UiTable 布局：名称/大小/修改时间；内容超宽时横向滚动） -->
    <UiScrollArea as-child axis="both">
      <div
        class="min-h-0 flex-1"
        aria-label="本地文件列表"
        @contextmenu="emit('blankContext', $event)"
      >
        <div v-if="loading" class="py-[16px] text-center text-caption text-text-muted">读取中…</div>
        <UiTable v-else :framed="false" :styled="false" table-class="w-max min-w-full text-body-sm">
          <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
            <tr
              class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
            >
              <UiTableCell as="th" class="px-[12px] py-[8px]">名称</UiTableCell>
              <UiTableCell as="th" class="w-[90px] px-[12px] py-[8px]">大小</UiTableCell>
              <UiTableCell as="th" class="w-[132px] px-[12px] py-[8px]">修改时间</UiTableCell>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="file in files"
              :key="file.path"
              class="cursor-pointer border-b border-border/50 transition-colors dark:border-border-dark/50"
              :class="
                selectedPaths?.has(file.path)
                  ? 'bg-tertiary-soft shadow-[inset_4px_0_0_0_#F0562C] dark:bg-tertiary-soft-dark'
                  : 'hover:bg-border dark:hover:bg-border-dark'
              "
              @click="onRowClick($event, file)"
              @dblclick="open(file)"
              @contextmenu.stop="emit('rowContext', $event, file)"
              @pointerdown="emit('rowPointerDown', $event, file)"
            >
              <UiTableCell content="technical" class="whitespace-nowrap px-[12px] py-[7px]">
                <UiTooltip :content="file.path">
                  <span>
                    <span class="mr-[6px]">{{ file.isDir ? '📁' : '📄' }}</span>
                    <span :class="{ 'font-medium': file.isDir }">{{ file.name }}</span>
                  </span>
                </UiTooltip>
              </UiTableCell>
              <UiTableCell content="numeric" class="whitespace-nowrap px-[12px] py-[7px]">{{
                file.isDir ? '-' : formatBytes(file.size)
              }}</UiTableCell>
              <UiTableCell content="numeric" class="whitespace-nowrap px-[12px] py-[7px]">{{
                formatTime(file.modifiedAt)
              }}</UiTableCell>
            </tr>
          </tbody>
        </UiTable>
      </div>
    </UiScrollArea>

    <!-- 状态栏 -->
    <UiStatusBar size="md">
      <span class="min-w-0 flex-1 truncate font-mono">{{ atDrives ? '此电脑' : currentPath }}</span>
      <template #trailing>
        <span>{{ files.length }} 项</span>
      </template>
    </UiStatusBar>
  </div>
</template>
