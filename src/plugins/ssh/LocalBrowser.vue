<script setup lang="ts">
/**
 * LocalBrowser · 双栏文件管理的本地侧（目录浏览 + 选中）
 * ssh_local_list 读目录；双击进入目录；选中文件后由中间列按钮发起上传。
 */
import { computed, onMounted, ref } from 'vue'
import type { RemoteFile } from './contracts'
import { ipc } from './ipc'
import { formatBytes, formatTime } from './useSsh'
import PathBreadcrumbs from './PathBreadcrumbs.vue'
import { UiIcon, UiIconButton, UiListRow } from '@/core/ui'

const props = defineProps<{
  /** 初始目录（默认取设置里的默认下载目录） */
  initialPath: string
}>()

const emit = defineEmits<{
  /** 双击文件 / 选中变化（供外部读取选中项） */
  (e: 'select', file: RemoteFile | null): void
  (e: 'error', message: string): void
}>()

/** 初始路径统一为反斜杠（设置里可能是 C:/ 正斜杠写法） */
const initialDir = props.initialPath.replace(/\//g, '\\')
const currentPath = ref(initialDir)
const files = ref<RemoteFile[]>([])
const selected = ref<RemoteFile | null>(null)
const loading = ref(false)

const parentPath = computed(() => {
  const normalized = currentPath.value.replace(/[\\/]+$/, '')
  const index = Math.max(normalized.lastIndexOf('\\'), normalized.lastIndexOf('/'))
  if (index <= 0) return null
  return normalized.slice(0, index)
})

async function navigate(path: string) {
  loading.value = true
  try {
    const result = await ipc.sshLocalList(path)
    if (!result.ok) {
      emit('error', result.error ?? '读取目录失败')
      return
    }
    currentPath.value = path
    files.value = result.files
    selected.value = null
    emit('select', null)
  } catch (error) {
    emit('error', String(error))
  } finally {
    loading.value = false
  }
}

function goUp() {
  if (parentPath.value) void navigate(parentPath.value)
}

function open(file: RemoteFile) {
  if (file.isDir) void navigate(file.path)
  return
}

function select(file: RemoteFile) {
  selected.value = file
  emit('select', file)
}

onMounted(() => void navigate(currentPath.value))

defineExpose({
  /** 当前目录（上传时用不到——上传用选中文件完整路径；保留给"上传到此目录"扩展） */
  currentPath,
  /** 重新加载当前目录 */
  refresh: () => navigate(currentPath.value),
})
</script>

<template>
  <div class="flex min-h-0 flex-col border-l border-border dark:border-border-dark">
    <!-- 工具栏：上级 + 面包屑路径（与远程侧同款）+ 刷新 -->
    <div
      class="flex shrink-0 items-center gap-[4px] border-b border-border px-[8px] py-[6px] dark:border-border-dark"
    >
      <UiIconButton label="上级" size="sm" title="上级目录" :disabled="!parentPath" @click="goUp">
        <UiIcon name="arrow-up" :size="14" />
      </UiIconButton>
      <PathBreadcrumbs
        class="min-w-0 flex-1"
        :path="currentPath"
        separator="\\"
        @navigate="navigate"
      />
      <UiIconButton label="刷新" size="sm" title="刷新" @click="navigate(currentPath)">
        <UiIcon name="refresh" :size="14" />
      </UiIconButton>
    </div>

    <!-- 列表 -->
    <div class="min-h-0 flex-1 overflow-y-auto" aria-label="本地文件列表">
      <div v-if="loading" class="py-[16px] text-center text-caption text-text-muted">读取中…</div>
      <UiListRow
        v-for="file in files"
        :key="file.path"
        size="sm"
        :active="selected?.path === file.path"
        :title="file.path"
        @click="select(file)"
        @dblclick="open(file)"
      >
        <span class="mr-[6px]">{{ file.isDir ? '📁' : '📄' }}</span>
        <span class="min-w-0 flex-1 truncate text-body-sm" :class="file.isDir ? 'font-medium' : ''">
          {{ file.name }}
        </span>
        <span class="shrink-0 text-caption text-text-muted">
          {{ file.isDir ? '-' : formatBytes(file.size) }}
        </span>
        <span class="hidden shrink-0 text-caption text-text-muted xl:inline">
          {{ formatTime(file.modifiedAt) }}
        </span>
      </UiListRow>
    </div>

    <!-- 状态栏 -->
    <div
      class="flex shrink-0 items-center gap-[8px] border-t border-border px-[10px] py-[4px] text-caption text-text-muted dark:border-border-dark"
    >
      <span class="min-w-0 flex-1 truncate font-mono">{{ currentPath }}</span>
      <span>{{ files.length }} 项</span>
    </div>
  </div>
</template>
