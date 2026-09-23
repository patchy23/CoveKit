<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  UiBottomPanel,
  UiButton,
  UiSelect,
  UiSearchInput,
  UiScrollArea,
  UiTable,
  UiTableCell,
  UiAlert,
} from '@/core/ui'
import { formatBytes } from '../connection/useSsh'
import type { TransferItem } from './useFileTransfer'
const props = defineProps<{ open: boolean; items: TransferItem[] }>()
defineEmits<{
  'update:open': [value: boolean]
  cancel: [id: string]
  clear: []
  retry: [item: TransferItem]
  locate: [item: TransferItem]
}>()
const filter = ref('all'),
  query = ref('')
const options = [
  { value: 'all', label: '全部' },
  { value: 'running', label: '进行中' },
  { value: 'failed', label: '失败' },
  { value: 'done', label: '已完成' },
]
const rows = computed(() =>
  props.items
    .filter(
      (v) =>
        (v.label + v.localPath + v.remotePath).toLowerCase().includes(query.value.toLowerCase()) &&
        (filter.value === 'all' ||
          (filter.value === 'running' && !v.done) ||
          (filter.value === 'failed' && !!v.error) ||
          (filter.value === 'done' && v.done && !v.error))
    )
    .reverse()
)
</script>
<template>
  <UiBottomPanel :open="open" title="传输任务" @update:open="$emit('update:open', $event)">
    <template #actions
      ><UiSearchInput v-model="query" placeholder="搜索文件或路径" size="sm" /><UiSelect
        v-model="filter"
        :options="options"
        size="sm"
      /><UiButton
        size="xs"
        variant="ghost"
        @click="items.filter((v) => !v.done).forEach((v) => $emit('cancel', v.id))"
        >取消全部</UiButton
      ><UiButton size="xs" variant="ghost" @click="$emit('clear')">清除已结束</UiButton></template
    >
    <UiScrollArea class="min-h-0 flex-1" axis="both">
      <UiTable density="compact"
        ><thead>
          <tr>
            <UiTableCell as="th">方向 / 文件</UiTableCell
            ><UiTableCell as="th">进度 / 状态</UiTableCell
            ><UiTableCell as="th">速度</UiTableCell
            ><UiTableCell as="th" content="action">操作</UiTableCell>
          </tr>
        </thead>
        <tbody>
          <tr v-for="item in rows" :key="item.id">
            <UiTableCell
              ><div>{{ item.kind === 'upload' ? '上传' : '下载' }} · {{ item.label }}</div>
              <div
                class="select-text font-mono text-caption text-secondary dark:text-secondary-dark"
              >
                {{ item.localPath }} ↔ {{ item.remotePath }}
              </div>
              <UiAlert v-if="item.error" tone="danger" size="xs">{{
                item.error
              }}</UiAlert></UiTableCell
            >
            <UiTableCell
              >{{
                item.done
                  ? item.cancelling
                    ? '已取消'
                    : item.error
                      ? '失败'
                      : '已完成'
                  : item.cancelling
                    ? '取消中'
                    : item.preparing
                      ? '正在准备'
                      : '传输中'
              }}
              <div>
                {{ item.total > 0 ? Math.floor((item.transferred / item.total) * 100) + '% · ' : ''
                }}{{ formatBytes(item.transferred) }} /
                {{ item.total > 0 ? formatBytes(item.total) : '正在统计' }}
              </div></UiTableCell
            >
            <UiTableCell>{{
              !item.done && item.speed > 0 ? formatBytes(item.speed) + '/s' : '—'
            }}</UiTableCell>
            <UiTableCell content="action"
              ><UiButton
                v-if="!item.done"
                size="xs"
                variant="ghost"
                :disabled="item.cancelling"
                @click="$emit('cancel', item.id)"
                >取消</UiButton
              ><UiButton
                v-else-if="item.error"
                size="xs"
                variant="ghost"
                @click="$emit('retry', item)"
                >重新传输</UiButton
              ><UiButton size="xs" variant="ghost" @click="$emit('locate', item)"
                >定位</UiButton
              ></UiTableCell
            >
          </tr>
          <tr v-if="!rows.length">
            <UiTableCell colspan="4">暂无匹配的传输任务</UiTableCell>
          </tr>
        </tbody>
      </UiTable>
    </UiScrollArea>
  </UiBottomPanel>
</template>
