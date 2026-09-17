<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { UiBadge, UiTable, UiTableCell } from '@/core/ui'
import AppIcon from '@/features/ui/AppIcon.vue'
import type { CredentialKind, CredentialSummary } from '@/core/ipc/contracts'
import { KIND_LABEL, KIND_TONE, formatTimestamp } from '@/core/vault/useVault'

defineProps<{
  items: CredentialSummary[]
  loading: boolean
  query: string
  kindFilter: CredentialKind | 'all'
}>()
const emit = defineEmits<{ context: [event: MouseEvent, item: CredentialSummary] }>()
</script>

<template>
  <UiScrollArea as-child axis="both">
    <div class="min-h-0 flex-1">
      <UiTable v-if="items.length" density="compact" :hoverable="true">
        <thead>
          <tr>
            <UiTableCell as="th">名称</UiTableCell>
            <UiTableCell as="th" class="w-[110px]">类型</UiTableCell>
            <UiTableCell as="th">摘要</UiTableCell>
            <UiTableCell as="th">备注</UiTableCell>
            <UiTableCell as="th" class="w-[130px]">更新时间</UiTableCell>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="item in items"
            :key="item.id"
            class="cursor-context-menu"
            @contextmenu="emit('context', $event, item)"
          >
            <UiTableCell>{{ item.name }}</UiTableCell>
            <UiTableCell>
              <UiBadge :tone="KIND_TONE[item.kind]" size="xs">{{ KIND_LABEL[item.kind] }}</UiBadge>
            </UiTableCell>
            <UiTableCell content="technical">{{ item.masked }}</UiTableCell>
            <UiTableCell>{{ item.note || '—' }}</UiTableCell>
            <UiTableCell content="technical">{{ formatTimestamp(item.updatedAt) }}</UiTableCell>
          </tr>
        </tbody>
      </UiTable>
      <div
        v-else
        class="flex flex-col items-center gap-[8px] py-[60px] text-caption text-text-muted dark:text-text-muted-dark"
      >
        <AppIcon name="lock" :size="28" />
        <p>
          {{
            loading
              ? '加载中…'
              : query || kindFilter !== 'all'
                ? '无匹配凭证'
                : '暂无凭证，点击右上角新建'
          }}
        </p>
      </div>
    </div>
  </UiScrollArea>
</template>
