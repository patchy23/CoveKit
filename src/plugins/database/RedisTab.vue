<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * Redis 键详情页签：键名/类型/TTL/值（只读展示）
 */
import { computed } from 'vue'
import { UiBadge, UiButton, UiTable, UiTableCell } from '@/core/ui'
import type { useDatabase } from './useDatabase'

const props = defineProps<{
  db: ReturnType<typeof useDatabase>
}>()

const { db } = props

const state = computed(() => db.queryState.value)

const toneOf = (kind: string) => {
  switch (kind) {
    case 'string':
      return 'success' as const
    case 'list':
    case 'set':
    case 'zset':
    case 'hash':
    case 'stream':
      return 'info' as const
    default:
      return 'neutral' as const
  }
}
</script>

<template>
  <UiScrollArea as-child axis="both">
    <div class="min-h-0 flex-1 p-[6px]">
      <div class="mb-[6px]">
        <div class="flex items-center gap-[8px]">
          <UiButton
            size="xs"
            :disabled="state.status === 'running'"
            @click="
              db.loadRedisKeyInfo(db.activeTabId.value, db.activeTabContext.value.table ?? '')
            "
            >刷新</UiButton
          >
          <h2 class="font-mono text-body-sm font-semibold text-primary dark:text-primary-dark">
            {{ state.rows[0]?.[0] ?? '' }}
          </h2>
          <UiBadge v-if="state.rows[0]?.[1]" :tone="toneOf(state.rows[0][1])" size="xs">
            {{ state.rows[0][1] }}
          </UiBadge>
        </div>
        <p class="mt-[2px] text-caption text-secondary dark:text-secondary-dark">
          {{ db.activeTabConnection.value?.label ?? '' }} · Redis 键
        </p>
      </div>

      <UiTable v-if="state.rows.length" density="compact" :hoverable="true">
        <thead>
          <tr>
            <UiTableCell as="th">键</UiTableCell>
            <UiTableCell as="th">类型</UiTableCell>
            <UiTableCell as="th">TTL</UiTableCell>
            <UiTableCell as="th">值</UiTableCell>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, index) in state.rows" :key="index">
            <UiTableCell content="technical">{{ row[0] }}</UiTableCell>
            <UiTableCell>
              <UiBadge :tone="toneOf(row[1])" size="xs">{{ row[1] }}</UiBadge>
            </UiTableCell>
            <UiTableCell content="technical">{{ row[2] }}</UiTableCell>
            <UiTableCell content="technical" class="max-w-[480px] break-all whitespace-pre-wrap">
              {{ row[3] }}
            </UiTableCell>
          </tr>
        </tbody>
      </UiTable>

      <div
        v-else
        class="py-[40px] text-center text-caption text-text-muted dark:text-text-muted-dark"
      >
        {{
          state.status === 'error'
            ? state.error
            : state.status === 'running'
              ? '加载中…'
              : '键不存在或已过期'
        }}
      </div>
    </div>
  </UiScrollArea>
</template>
