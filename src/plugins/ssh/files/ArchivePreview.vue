<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  UiFloatingWindow,
  UiToolbar,
  UiButton,
  UiSearchInput,
  UiAlert,
  UiScrollArea,
  UiTable,
  UiTableCell,
  UiPagination,
  UiStatusBar,
} from '@/core/ui'
import type { ArchiveEntry } from '../contracts'
import { formatBytes, formatTime } from '../connection/useSsh'
const props = defineProps<{ path: string; entries: ArchiveEntry[]; busy: boolean; error: string }>()
defineEmits<{ close: []; minimize: []; cancel: []; reload: []; extract: []; download: [] }>()
const directory = ref(''),
  query = ref(''),
  page = ref(1)
watch([directory, query], () => {
  page.value = 1
})
const rows = computed(() => {
  if (query.value)
    return props.entries.filter((entry) =>
      entry.path.toLowerCase().includes(query.value.toLowerCase())
    )
  const result = new Map<string, ArchiveEntry>()
  for (const entry of props.entries) {
    if (!entry.path.startsWith(directory.value)) continue
    const relative = entry.path.slice(directory.value.length)
    if (!relative) continue
    const segment = relative.split('/')[0],
      path = directory.value + segment
    if (relative.includes('/')) {
      if (!result.has(path)) result.set(path, { path, isDir: true, size: null, modifiedAt: 0 })
    } else result.set(path, entry)
  }
  return [...result.values()].sort(
    (a, b) => Number(b.isDir) - Number(a.isDir) || a.path.localeCompare(b.path)
  )
})
const pages = computed(() => Math.max(1, Math.ceil(rows.value.length / 200)))
const visibleRows = computed(() => rows.value.slice((page.value - 1) * 200, page.value * 200))
function enter(path: string) {
  directory.value = path + '/'
  query.value = ''
}
function up() {
  const parts = directory.value.split('/').filter(Boolean)
  parts.pop()
  directory.value = parts.length ? parts.join('/') + '/' : ''
}
</script>
<template>
  <UiFloatingWindow title="压缩包预览" :width="820" :height="560" @close="$emit('close')">
    <UiToolbar bordered>
      <UiButton size="xs" variant="ghost" :disabled="!directory" @click="up">上一级</UiButton>
      <span class="min-w-0 flex-1 truncate select-text font-mono text-caption"
        >/{{ directory }}</span
      >
      <UiButton size="xs" variant="ghost" :disabled="busy" @click="$emit('reload')"
        >重新读取</UiButton
      >
      <UiButton size="xs" variant="ghost" @click="$emit('minimize')">收起</UiButton>
    </UiToolbar>
    <div class="flex items-center gap-sm p-sm">
      <UiSearchInput
        v-model="query"
        size="sm"
        class="min-w-0 flex-1"
        :placeholder="busy ? '搜索已扫描条目' : '搜索包内路径'"
      />
      <UiButton size="sm" variant="ghost" @click="$emit('download')">下载压缩包</UiButton>
      <UiButton size="sm" variant="ghost" @click="$emit('extract')">解压到服务器</UiButton>
    </div>
    <p class="truncate select-text px-sm text-caption text-secondary dark:text-secondary-dark">
      {{ path }}
    </p>
    <UiAlert v-if="error" tone="warning" size="sm">{{ error }}</UiAlert>
    <UiScrollArea class="min-h-0 flex-1" axis="both">
      <UiTable density="compact"
        ><thead>
          <tr>
            <UiTableCell as="th">名称</UiTableCell
            ><UiTableCell as="th">大小</UiTableCell
            ><UiTableCell as="th">修改时间</UiTableCell>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(entry, index) in visibleRows" :key="entry.path + index">
            <UiTableCell content="technical"
              ><UiButton v-if="entry.isDir" size="xs" variant="ghost" @click="enter(entry.path)"
                >{{ query ? entry.path : entry.path.slice(directory.length) }}/</UiButton
              ><span v-else>{{
                query ? entry.path : entry.path.slice(directory.length)
              }}</span></UiTableCell
            >
            <UiTableCell content="numeric">{{
              entry.isDir ? '—' : entry.size === null ? '未知' : formatBytes(entry.size)
            }}</UiTableCell>
            <UiTableCell content="numeric">{{
              entry.modifiedAt ? formatTime(entry.modifiedAt) : '—'
            }}</UiTableCell>
          </tr>
          <tr v-if="!visibleRows.length">
            <UiTableCell colspan="3">{{ busy ? '正在扫描归档目录…' : '暂无匹配条目' }}</UiTableCell>
          </tr>
        </tbody>
      </UiTable>
    </UiScrollArea>
    <UiStatusBar
      ><span>{{ entries.length }} 条 · {{ busy ? '扫描中' : '扫描结束' }}</span
      ><template #trailing
        ><UiButton v-if="busy" size="xs" variant="ghost" @click="$emit('cancel')">取消扫描</UiButton
        ><UiPagination v-model="page" :total-pages="pages" size="xs" /></template
    ></UiStatusBar>
  </UiFloatingWindow>
</template>
